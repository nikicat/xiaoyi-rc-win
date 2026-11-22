//! The one orchestrator that wires the layers, without a general graph
//! framework (ADR 7, "Layers").

use crate::arbiter;
use crate::bindings::Config;
use crate::device::{InputReport, KeyReport};
use crate::event::{History, Tracker};
use crate::output::Output;
use crate::time::Time;

/// Turns the remote's input reports into the ones that Windows gets, by
/// recognizing the gestures of [`Config`] over a history of button events
/// (ADR 7, "Layers").
#[derive(Debug, Default)]
pub struct Recognizer {
    tracker: Tracker,
    history: History,
    output: Output,
    deadline: Option<Time>,
}

impl Recognizer {
    /// Queues the reports that Windows gets from an input report that came at
    /// `now`. They are the taps of the gestures that fire, and the release of each
    /// key whose button the report releases.
    pub fn report(&mut self, now: Time, report: &InputReport, config: &Config) {
        // A series that ended by now is decided before this report's events join one.
        self.expire(now, config);
        let events = self.tracker.track(report.buttons(), now);
        self.output.input(&events);
        self.history.push(events);
        self.expire(now, config);
    }

    /// Fires the gestures that are decided by `now`, and queues their reports.
    pub fn expire(&mut self, now: Time, config: &Config) {
        let gestures = config.gestures();
        let decision = arbiter::decide(&gestures, &mut self.history, now);
        decision
            .firings
            .into_iter()
            .for_each(|f| self.output.fire(f));
        self.deadline = decision.deadline;
    }

    /// Returns the time by which [`Self::expire`] must run, or None when no timer
    /// is needed (ADR 7, "Gestures").
    pub fn deadline(&self) -> Option<Time> {
        self.deadline
    }

    pub(crate) fn pop(&mut self) -> Option<KeyReport> {
        self.output.pop()
    }

    pub(crate) fn queued(&self) -> usize {
        self.output.queued()
    }

    /// Returns a report of the keys of the last queued report. Windows holds those
    /// keys once the queue is empty.
    pub(crate) fn held_report(&self) -> KeyReport {
        self.output.held_report()
    }

    /// Forgets every event, every queued report and the keys held for Windows.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::Binding;
    use crate::device::Usage;
    use crate::device::testing::*;
    use crate::time::testing::{WINDOW, ms};

    const BIG: Usage = VOLUME_UP;
    const SMALL: Usage = VOLUME_DOWN;

    /// A config with `bindings` from patterns to keys, and the window that the
    /// tests' times assume.
    fn bound(bindings: &[(&[Usage], Usage)]) -> Config {
        let mut config = Config {
            window: WINDOW,
            ..Config::default()
        };
        for &(pattern, to) in bindings {
            config
                .bindings
                .set(Binding::new(pattern, Some(to)).unwrap());
        }
        config
    }

    struct Remote {
        recognizer: Recognizer,
        config: Config,
    }

    impl Remote {
        fn new(config: Config) -> Self {
            Self {
                recognizer: Recognizer::default(),
                config,
            }
        }

        fn at(&mut self, t: u64, usages: &[Usage]) -> &mut Self {
            self.recognizer.report(ms(t), &input(usages), &self.config);
            self
        }

        fn expire(&mut self, t: u64) -> &mut Self {
            self.recognizer.expire(ms(t), &self.config);
            self
        }

        fn sent(&mut self) -> Vec<KeyReport> {
            std::iter::from_fn(|| self.recognizer.pop()).collect()
        }
    }

    #[test]
    fn unbound_buttons_are_instant() {
        let mut remote = Remote::new(Config::default());
        assert_eq!(remote.at(0, &[BIG]).sent(), [report(&[BIG])]);
        assert_eq!(remote.at(100, &[]).sent(), [report(&[])]);
        assert_eq!(remote.recognizer.deadline(), None);
    }

    #[test]
    fn instant_buttons_report_their_bound_key() {
        let mut config = Config::default();
        config
            .bindings
            .set(Binding::new(&[BIG], Some(PLAY_PAUSE)).unwrap());
        let mut remote = Remote::new(config);
        assert_eq!(remote.at(0, &[BIG]).sent(), [report(&[PLAY_PAUSE])]);
    }

    #[test]
    fn repeated_reports_send_nothing_new() {
        let mut remote = Remote::new(Config::default());
        remote.at(0, &[BIG]).sent();
        assert!(remote.at(50, &[BIG]).sent().is_empty());
    }

    #[test]
    fn an_uncontested_button_holds_its_key() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        assert_eq!(remote.at(0, &[SMALL]).sent(), [report(&[SMALL])]);
        assert!(remote.expire(2000).sent().is_empty());
        assert_eq!(remote.at(2100, &[]).sent(), [report(&[])]);
        assert_eq!(remote.at(2150, &[SMALL]).sent(), [report(&[SMALL])]);
        assert_eq!(remote.at(2200, &[]).sent(), [report(&[])]);
    }

    #[test]
    fn single_click_waits_for_the_window() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        assert!(remote.at(0, &[BIG]).sent().is_empty());
        assert!(remote.at(100, &[]).sent().is_empty());
        assert_eq!(remote.recognizer.deadline(), Some(ms(400)));
        assert!(remote.expire(399).sent().is_empty());
        assert_eq!(remote.expire(400).sent(), [report(&[BIG]), report(&[])]);
        assert_eq!(remote.recognizer.deadline(), None);
    }

    #[test]
    fn single_click_sends_its_bound_key() {
        let mut config = bound(&[(&[BIG, BIG], NEXT)]);
        config
            .bindings
            .set(Binding::new(&[BIG], Some(PLAY_PAUSE)).unwrap());
        let mut remote = Remote::new(config);
        remote.at(0, &[BIG]).at(100, &[]);
        assert_eq!(
            remote.expire(400).sent(),
            [report(&[PLAY_PAUSE]), report(&[])]
        );
    }

    #[test]
    fn double_click_waits_for_the_window() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        remote.at(0, &[BIG]).at(100, &[]).at(250, &[BIG]);
        assert!(remote.at(350, &[]).sent().is_empty());
        assert_eq!(remote.recognizer.deadline(), Some(ms(650)));
        assert_eq!(remote.expire(650).sent(), [report(&[NEXT]), report(&[])]);
        assert_eq!(remote.recognizer.deadline(), None);
    }

    #[test]
    fn triple_click_sends_its_binding() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT), (&[BIG, BIG, BIG], PREVIOUS)]));
        remote
            .at(0, &[BIG])
            .at(100, &[])
            .at(250, &[BIG])
            .at(350, &[])
            .at(500, &[BIG])
            .at(600, &[]);
        assert!(remote.expire(899).sent().is_empty());
        assert_eq!(
            remote.expire(900).sent(),
            [report(&[PREVIOUS]), report(&[])]
        );
    }

    #[test]
    fn double_click_waits_while_a_triple_is_possible() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT), (&[BIG, BIG, BIG], PREVIOUS)]));
        remote
            .at(0, &[BIG])
            .at(100, &[])
            .at(250, &[BIG])
            .at(350, &[]);
        assert_eq!(remote.expire(650).sent(), [report(&[NEXT]), report(&[])]);
    }

    #[test]
    fn clicks_past_the_bindings_send_nothing() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        remote
            .at(0, &[BIG])
            .at(100, &[])
            .at(250, &[BIG])
            .at(350, &[])
            .at(450, &[BIG])
            .at(550, &[]);
        assert!(remote.expire(850).sent().is_empty());
        assert_eq!(remote.recognizer.deadline(), None);
    }

    #[test]
    fn unbound_count_sends_nothing() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG, BIG], PREVIOUS)]));
        remote
            .at(0, &[BIG])
            .at(100, &[])
            .at(250, &[BIG])
            .at(350, &[]);
        assert!(remote.expire(650).sent().is_empty());
    }

    #[test]
    fn clicks_after_the_window_start_over() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        remote.at(0, &[BIG]).at(100, &[]);
        assert_eq!(remote.at(500, &[BIG]).sent(), [report(&[BIG]), report(&[])]);
        assert!(remote.at(600, &[]).sent().is_empty());
        assert_eq!(remote.recognizer.deadline(), Some(ms(900)));
    }

    #[test]
    fn press_at_the_window_end_ends_the_series_without_the_timer() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        remote.at(0, &[BIG]).at(100, &[]);
        assert_eq!(remote.at(400, &[BIG]).sent(), [report(&[BIG]), report(&[])]);
        remote.at(500, &[]).at(600, &[BIG]).at(700, &[]);
        assert_eq!(remote.expire(1000).sent(), [report(&[NEXT]), report(&[])]);
    }

    #[test]
    fn hold_is_one_click() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        assert!(remote.at(0, &[BIG]).expire(2000).sent().is_empty());
        assert_eq!(remote.recognizer.deadline(), None);
        assert!(remote.at(2100, &[]).sent().is_empty());
        assert_eq!(remote.expire(2400).sent(), [report(&[BIG]), report(&[])]);
    }

    #[test]
    fn instant_button_held_through_a_tap() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        assert_eq!(remote.at(0, &[SMALL]).sent(), [report(&[SMALL])]);
        assert!(remote.at(10, &[SMALL, BIG]).sent().is_empty());
        remote.at(100, &[SMALL]).at(250, &[SMALL, BIG]);
        assert!(remote.at(350, &[SMALL]).sent().is_empty());
        assert_eq!(
            remote.expire(650).sent(),
            [report(&[SMALL, NEXT]), report(&[SMALL])]
        );
        assert_eq!(remote.at(700, &[]).sent(), [report(&[])]);
    }

    #[test]
    fn buttons_click_independently() {
        let mut config = bound(&[(&[BIG, BIG], NEXT)]);
        config
            .bindings
            .set(Binding::new(&[SMALL, SMALL], Some(MUTE)).unwrap());
        let mut remote = Remote::new(config);
        remote
            .at(0, &[BIG])
            .at(10, &[BIG, SMALL])
            .at(100, &[SMALL])
            .at(110, &[]);
        assert_eq!(remote.recognizer.deadline(), Some(ms(400)));
        remote.at(200, &[SMALL]);
        assert!(remote.at(300, &[]).sent().is_empty());
        assert_eq!(remote.expire(400).sent(), [report(&[BIG]), report(&[])]);
        assert_eq!(remote.expire(600).sent(), [report(&[MUTE]), report(&[])]);
    }

    #[test]
    fn single_click_can_report_no_key() {
        let mut config = bound(&[(&[BIG, BIG], NEXT)]);
        config.bindings.set(Binding::new(&[BIG], None).unwrap());
        let mut remote = Remote::new(config);
        remote.at(0, &[BIG]).at(100, &[]);
        assert!(remote.expire(400).sent().is_empty());
    }

    #[test]
    fn taps_of_two_buttons_form_one_gesture() {
        let mut remote = Remote::new(bound(&[(&[BIG, SMALL], PLAY_PAUSE)]));
        remote
            .at(0, &[BIG])
            .at(100, &[])
            .at(200, &[SMALL])
            .at(300, &[]);
        assert_eq!(
            remote.expire(600).sent(),
            [report(&[PLAY_PAUSE]), report(&[])]
        );
        remote.at(1000, &[SMALL]).at(1100, &[]);
        assert_eq!(remote.expire(1400).sent(), [report(&[SMALL]), report(&[])]);
    }

    #[test]
    fn reports_to_windows_have_padding_bits_of_0() {
        let mut remote = Remote::new(Config::default());
        remote
            .recognizer
            .report(ms(0), &padded_input(&[BIG]), &remote.config);
        assert_eq!(remote.sent(), [report(&[BIG])]);
    }

    #[test]
    fn reset_forgets_events_and_queue() {
        let mut remote = Remote::new(bound(&[(&[BIG, BIG], NEXT)]));
        remote
            .at(0, &[SMALL])
            .at(10, &[SMALL, BIG])
            .at(100, &[SMALL]);
        remote.recognizer.reset();
        assert_eq!(remote.recognizer.queued(), 0);
        assert_eq!(remote.recognizer.deadline(), None);
        assert_eq!(remote.recognizer.held_report(), report(&[]));
    }
}
