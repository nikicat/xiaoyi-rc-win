//! One completer at a time serves hidclass's reads (ADR 7, "The read path").

use std::collections::VecDeque;
use std::mem;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::bindings::Config;
use crate::device::{InputReport, KeyReport};
use crate::recognizer::Recognizer;
use crate::time::Time;

/// A handle that cancels a read down below.
pub trait Token {
    type Id: PartialEq;

    /// Returns the id of the read that this token cancels.
    fn id(&self) -> Self::Id;
}

/// The read state, and the one way in to it: [`Self::run`].
pub struct SerialCompleter<R, T: Token> {
    /// Nothing is completed, sent, cancelled or armed under this lock (ADR 7,
    /// "The read path").
    state: Mutex<ReadState<R, T>>,
}

/// The state of the read path. `R` is one of hidclass's reads, and `T` cancels
/// a read down below. Its public methods record work for the completer.
pub struct ReadState<R, T: Token> {
    recognizer: Recognizer,
    /// The reads that wait to carry a report up or to go down, oldest first.
    waiting: VecDeque<Waiting<R>>,
    /// The reads down below.
    below: Vec<Below<T>>,
    /// The deadline that the window timer was last started for, or None when
    /// it has fired since.
    armed: Option<Time>,
    completer: Completer,
}

/// What the completer does next, without the lock.
#[derive(Debug, PartialEq)]
pub enum Action<R, T> {
    /// Completes the read with the report.
    Complete(R, KeyReport),
    /// Sends the read down to wait for the remote's next report.
    Send(R),
    /// Cancels the read below, so that it carries a queued report up.
    Cancel(T),
    /// Starts the window timer to fire at the deadline.
    Arm(Time),
}

/// A read that waits to be served, by what it does when no report is queued.
enum Waiting<R> {
    /// hidclass's new read, or one that brought a report: it goes down.
    Fresh(R),
    /// A read that a flush cancelled: it goes up with the held keys (ADR 7,
    /// "The read path").
    Flushed(R),
}

struct Below<T: Token> {
    id: T::Id,
    phase: Phase<T>,
}

/// Where a read below stands.
enum Phase<T> {
    /// The read is going down, and it cannot be cancelled until the send
    /// returns.
    Sending(T),
    /// The read is down below, and a flush can cancel it.
    Sent(T),
    /// A flush cancelled the read to carry a queued report up.
    Flushing,
}

/// Whether a caller is serving the reads.
enum Completer {
    Idle,
    Running,
}

impl<R, T: Token> Default for SerialCompleter<R, T> {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
        }
    }
}

impl<R, T: Token> SerialCompleter<R, T> {
    /// Runs `record` under the read state's lock and returns its result, then
    /// performs the actions that follow unless another caller is the completer
    /// (ADR 7, "The read path"). Calling run from `record` deadlocks, and `perform`
    /// may call it again.
    pub fn run<U>(
        &self,
        record: impl FnOnce(&mut ReadState<R, T>) -> U,
        mut perform: impl FnMut(Action<R, T>),
    ) -> U {
        let (recorded, mut action) = {
            let mut state = self.lock();
            let recorded = record(&mut state);
            (recorded, state.claim())
        };
        while let Some(next) = action {
            perform(next);
            action = self.lock().next_action();
        }
        recorded
    }

    fn lock(&self) -> MutexGuard<'_, ReadState<R, T>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<R, T: Token> Default for ReadState<R, T> {
    fn default() -> Self {
        Self {
            recognizer: Recognizer::default(),
            waiting: VecDeque::new(),
            below: Vec::new(),
            armed: None,
            completer: Completer::Idle,
        }
    }
}

impl<R, T: Token> ReadState<R, T> {
    /// Fires the gestures that are decided by `now`, since the window timer
    /// has fired.
    pub fn expire(&mut self, now: Time, config: &Config) {
        self.armed = None;
        self.recognizer.expire(now, config);
    }

    /// Takes hidclass's new read, which waits to be served.
    pub fn park(&mut self, read: R) {
        self.waiting.push_back(Waiting::Fresh(read));
    }

    /// Lists a read that is about to go down.
    pub fn sending(&mut self, token: T) {
        self.below.push(Below {
            id: token.id(),
            phase: Phase::Sending(token),
        });
    }

    /// Marks the read `id` as sent, so that a flush can cancel it, unless it has
    /// already come back.
    pub fn sent(&mut self, id: T::Id) {
        if let Some(below) = self.below.iter_mut().find(|below| below.id == id) {
            below.phase.sent();
        }
    }

    /// Takes back the read `id`, which brought the remote's input report that
    /// came at `now`, with the settings read for it. It waits to be served.
    pub fn came_back(
        &mut self,
        id: T::Id,
        read: R,
        now: Time,
        report: &InputReport,
        config: &Config,
    ) {
        self.recognizer.report(now, report, config);
        let waiting = match self.forget_phase(id) {
            Some(Phase::Flushing) => Waiting::Flushed(read),
            _ => Waiting::Fresh(read),
        };
        self.waiting.push_back(waiting);
    }

    /// Takes back the read `id`, which came back cancelled. A read that a flush
    /// cancelled waits to be served. Any other is handled as [`Self::failed`]
    /// is, and is returned.
    pub fn cancelled(&mut self, id: T::Id, read: R) -> Option<R> {
        if let Some(Phase::Flushing) = self.forget_phase(id) {
            self.waiting.push_back(Waiting::Flushed(read));
            return None;
        }
        self.recognizer.reset();
        Some(read)
    }

    /// Forgets the read `id`, which failed, and resets the gestures, since the
    /// remote's button state is then unknown (ADR 7, "The read path").
    pub fn failed(&mut self, id: T::Id) {
        self.forget(id);
        self.recognizer.reset();
    }

    /// Forgets the read `id`, which is no longer below.
    pub fn forget(&mut self, id: T::Id) {
        self.forget_phase(id);
    }

    /// Makes the caller the completer when none is running, and returns its
    /// first action. Returns None when another caller is the completer, which
    /// takes the work recorded so far, or when there is nothing to do.
    fn claim(&mut self) -> Option<Action<R, T>> {
        match self.completer {
            Completer::Running => None,
            Completer::Idle => self.next_action(),
        }
    }

    /// Returns the completer's next action. Returns None when nothing is left,
    /// and the completer is then idle.
    fn next_action(&mut self) -> Option<Action<R, T>> {
        let action = self.decide();
        self.completer = match action {
            Some(_) => Completer::Running,
            None => Completer::Idle,
        };
        action
    }

    /// Serves the oldest waiting read first, then cancels a read below for a
    /// queued report that no read carries up, then starts the window timer for
    /// a new deadline.
    fn decide(&mut self) -> Option<Action<R, T>> {
        if let Some(waiting) = self.waiting.pop_front() {
            return Some(self.serve(waiting));
        }
        if let Some(token) = self.flush() {
            return Some(Action::Cancel(token));
        }
        self.arm().map(Action::Arm)
    }

    /// Returns what the waiting read does: it carries the next queued report
    /// up, or else a flushed read carries the held keys up and any other goes
    /// down.
    fn serve(&mut self, waiting: Waiting<R>) -> Action<R, T> {
        match (waiting, self.recognizer.pop()) {
            (Waiting::Fresh(read) | Waiting::Flushed(read), Some(report)) => {
                Action::Complete(read, report)
            }
            (Waiting::Flushed(read), None) => Action::Complete(read, self.recognizer.held_report()),
            (Waiting::Fresh(read), None) => Action::Send(read),
        }
    }

    /// Moves a sent read to flushing when a queued report has no read to carry
    /// it up, and returns its token to cancel.
    fn flush(&mut self) -> Option<T> {
        let flushing = self
            .below
            .iter()
            .filter(|below| matches!(below.phase, Phase::Flushing))
            .count();
        if self.recognizer.queued() <= flushing {
            return None;
        }
        let below = self
            .below
            .iter_mut()
            .find(|below| matches!(below.phase, Phase::Sent(_)))?;
        let Phase::Sent(token) = mem::replace(&mut below.phase, Phase::Flushing) else {
            unreachable!("the read was found sent");
        };
        Some(token)
    }

    /// Returns the gestures' deadline when the window timer is not started for
    /// it yet, and counts the timer as started for it.
    fn arm(&mut self) -> Option<Time> {
        let deadline = self
            .recognizer
            .deadline()
            .filter(|&deadline| self.armed != Some(deadline))?;
        self.armed = Some(deadline);
        Some(deadline)
    }

    /// Forgets the read `id`. Returns the phase that it ended in, or None when
    /// it was not listed.
    fn forget_phase(&mut self, id: T::Id) -> Option<Phase<T>> {
        let index = self.below.iter().position(|below| below.id == id)?;
        Some(self.below.remove(index).phase)
    }
}

impl<T> Phase<T> {
    fn sent(&mut self) {
        *self = match mem::replace(self, Self::Flushing) {
            Self::Sending(token) => Self::Sent(token),
            other => other,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::Binding;
    use crate::device::Usage;
    use crate::device::testing::*;
    use crate::time::testing::ms;

    /// One of hidclass's reads.
    #[derive(Debug, PartialEq)]
    struct Read(u32);

    #[derive(Debug, PartialEq)]
    struct Tok(u32);

    impl Token for Tok {
        type Id = u32;

        fn id(&self) -> u32 {
            self.0
        }
    }

    type State = ReadState<Read, Tok>;

    type Reads = SerialCompleter<Read, Tok>;

    type ReadAction = Action<Read, Tok>;

    /// Runs `record`, and returns the actions that this caller performs as the
    /// completer.
    fn run(reads: &Reads, record: impl FnOnce(&mut State)) -> Vec<ReadAction> {
        let mut actions = Vec::new();
        reads.run(record, |action| actions.push(action));
        actions
    }

    /// Runs `record` while another caller is the completer, which takes its
    /// work.
    fn run_aside(reads: &Reads, record: impl FnOnce(&mut State)) {
        reads.run(record, |action| panic!("a second completer got {action:?}"));
    }

    /// Feeds the remote's report of `usages` at `t`, with no read that brings
    /// it. With no bindings, each change queues one report at once.
    fn remote(state: &mut State, t: u64, usages: &[Usage]) {
        state
            .recognizer
            .report(ms(t), &input(usages), &Config::default());
    }

    /// A config whose double click on Volume Up sends Next, so that a single
    /// tap of it waits for the window.
    fn double_click() -> Config {
        let mut config = Config::default();
        config
            .bindings
            .set(Binding::new(&[VOLUME_UP, VOLUME_UP], Some(NEXT)).unwrap());
        config
    }

    /// Sends reads `ids` down, as the completer does.
    fn below(reads: &Reads, ids: &[u32]) {
        for &id in ids {
            assert_eq!(run(reads, |s| s.park(Read(id))), [Action::Send(Read(id))]);
            let sent = run(reads, |s| {
                s.sending(Tok(id));
                s.sent(id);
            });
            assert_eq!(sent, []);
        }
    }

    #[test]
    fn queued_reports_are_served_in_order() {
        let reads = Reads::default();
        let queued = run(&reads, |s| {
            remote(s, 0, &[VOLUME_UP]);
            remote(s, 10, &[]);
        });
        assert_eq!(queued, []);
        let served = run(&reads, |s| {
            s.park(Read(1));
            s.park(Read(2));
        });
        assert_eq!(
            served,
            [
                Action::Complete(Read(1), report(&[VOLUME_UP])),
                Action::Complete(Read(2), report(&[])),
            ]
        );
    }

    #[test]
    fn a_read_with_nothing_queued_goes_down() {
        let reads = Reads::default();
        assert_eq!(run(&reads, |s| s.park(Read(1))), [Action::Send(Read(1))]);
    }

    #[test]
    fn a_read_that_comes_back_before_it_is_marked_sent_is_not_cancelled() {
        let reads = Reads::default();
        let actions = run(&reads, |s| {
            s.sending(Tok(1));
            s.came_back(1, Read(1), ms(0), &input(&[VOLUME_UP]), &Config::default());
            s.sent(1);
        });
        assert_eq!(actions, [Action::Complete(Read(1), report(&[VOLUME_UP]))]);
    }

    #[test]
    fn a_read_still_sending_is_not_cancelled() {
        let reads = Reads::default();
        let sending = run(&reads, |s| {
            s.sending(Tok(1));
            remote(s, 0, &[VOLUME_UP]);
        });
        assert_eq!(sending, []);
        assert_eq!(run(&reads, |s| s.sent(1)), [Action::Cancel(Tok(1))]);
    }

    #[test]
    fn flushes_are_limited_to_the_queued_reports() {
        let reads = Reads::default();
        below(&reads, &[1, 2, 3]);
        let flushed = run(&reads, |s| {
            remote(s, 0, &[VOLUME_UP]);
            remote(s, 10, &[]);
        });
        assert_eq!(flushed, [Action::Cancel(Tok(1)), Action::Cancel(Tok(2))]);
        assert_eq!(
            run(&reads, |s| remote(s, 20, &[VOLUME_DOWN])),
            [Action::Cancel(Tok(3))]
        );
    }

    #[test]
    fn a_flushed_read_carries_the_next_queued_report() {
        let reads = Reads::default();
        below(&reads, &[1]);
        assert_eq!(
            run(&reads, |s| remote(s, 0, &[VOLUME_UP])),
            [Action::Cancel(Tok(1))]
        );
        assert_eq!(
            run(&reads, |s| assert_eq!(s.cancelled(1, Read(1)), None)),
            [Action::Complete(Read(1), report(&[VOLUME_UP]))]
        );
    }

    #[test]
    fn a_flushed_read_carries_the_held_keys_when_nothing_is_queued() {
        let reads = Reads::default();
        below(&reads, &[1]);
        let mut actions = Vec::new();
        reads.run(
            |s| remote(s, 0, &[VOLUME_UP]),
            |action| {
                if let Action::Cancel(_) = action {
                    run_aside(&reads, |s| s.park(Read(2)));
                }
                actions.push(action);
            },
        );
        assert_eq!(
            actions,
            [
                Action::Cancel(Tok(1)),
                Action::Complete(Read(2), report(&[VOLUME_UP])),
            ]
        );
        assert_eq!(
            run(&reads, |s| assert_eq!(s.cancelled(1, Read(1)), None)),
            [Action::Complete(Read(1), report(&[VOLUME_UP]))]
        );
    }

    #[test]
    fn a_flushed_read_that_brings_a_report_still_goes_up() {
        let reads = Reads::default();
        below(&reads, &[1]);
        reads.run(
            |s| remote(s, 0, &[VOLUME_UP]),
            |action| {
                if let Action::Cancel(_) = action {
                    run_aside(&reads, |s| s.park(Read(2)));
                }
            },
        );
        let came_back = run(&reads, |s| {
            s.came_back(1, Read(1), ms(5), &input(&[VOLUME_UP]), &Config::default());
        });
        assert_eq!(came_back, [Action::Complete(Read(1), report(&[VOLUME_UP]))]);
    }

    #[test]
    fn a_read_cancelled_by_no_flush_goes_up_as_it_came_and_resets() {
        let reads = Reads::default();
        below(&reads, &[1]);
        let actions = run(&reads, |s| {
            remote(s, 0, &[VOLUME_UP]);
            assert_eq!(s.cancelled(1, Read(1)), Some(Read(1)));
        });
        assert_eq!(actions, []);
    }

    #[test]
    fn a_failed_read_resets_the_gestures() {
        let reads = Reads::default();
        below(&reads, &[1, 2]);
        let actions = run(&reads, |s| {
            remote(s, 0, &[VOLUME_UP]);
            s.failed(1);
            s.park(Read(3));
        });
        assert_eq!(actions, [Action::Send(Read(3))]);
    }

    #[test]
    fn a_read_parked_while_running_is_served_by_the_completer() {
        let reads = Reads::default();
        run(&reads, |s| {
            remote(s, 0, &[VOLUME_UP]);
            remote(s, 10, &[]);
        });
        let mut actions = Vec::new();
        reads.run(
            |s| s.park(Read(1)),
            |action| {
                // hidclass sends its next read from inside that completion.
                if action == Action::Complete(Read(1), report(&[VOLUME_UP])) {
                    run_aside(&reads, |s| s.park(Read(2)));
                }
                actions.push(action);
            },
        );
        assert_eq!(
            actions,
            [
                Action::Complete(Read(1), report(&[VOLUME_UP])),
                Action::Complete(Read(2), report(&[])),
            ]
        );
    }

    #[test]
    fn work_recorded_on_another_thread_while_a_completer_runs_is_served() {
        let reads = Reads::default();
        let mut actions = Vec::new();
        reads.run(
            |s| s.park(Read(1)),
            |action| {
                if action == Action::Send(Read(1)) {
                    std::thread::scope(|scope| {
                        scope.spawn(|| run_aside(&reads, |s| s.park(Read(2))));
                    });
                }
                actions.push(action);
            },
        );
        assert_eq!(actions, [Action::Send(Read(1)), Action::Send(Read(2))]);
    }

    #[test]
    fn the_completer_goes_idle_only_when_nothing_is_left() {
        let reads = Reads::default();
        assert_eq!(run(&reads, |_| ()), []);
        let mut actions = Vec::new();
        reads.run(
            |s| s.park(Read(1)),
            |action| {
                if action == Action::Send(Read(1)) {
                    run_aside(&reads, |s| s.park(Read(2)));
                }
                actions.push(action);
            },
        );
        assert_eq!(actions, [Action::Send(Read(1)), Action::Send(Read(2))]);
        assert_eq!(run(&reads, |s| s.park(Read(3))), [Action::Send(Read(3))]);
    }

    #[test]
    fn the_window_timer_is_armed_once_per_deadline() {
        let reads = Reads::default();
        let config = double_click();
        let deadline = ms(100) + Config::WINDOW;
        let tapped = run(&reads, |s| {
            for (t, usages) in [(0, &[VOLUME_UP][..]), (100, &[])] {
                s.recognizer.report(ms(t), &input(usages), &config);
            }
        });
        assert_eq!(tapped, [Action::Arm(deadline)]);
        assert_eq!(run(&reads, |_| ()), []);
        // The timer fired a little early, so the deadline stands.
        assert_eq!(
            run(&reads, |s| s.expire(ms(99) + Config::WINDOW, &config)),
            [Action::Arm(deadline)]
        );
    }
}
