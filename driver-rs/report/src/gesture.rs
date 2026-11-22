//! A gesture is a pattern over the history of events, with no state of its own,
//! that is asked again by time (ADR 7, "Gestures").

use std::time::Duration;

use crate::device::Button;
use crate::event::{Edge, Event, EventSet};
use crate::time::{Time, earliest};

/// A gesture's answer about the events in the history (ADR 7, "Gestures").
/// `claimed` holds the events that it matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The series cannot be this gesture.
    No,
    /// The series could still be this gesture, which wants to be asked again
    /// by `until`.
    Maybe {
        claimed: EventSet,
        until: Option<Time>,
    },
    /// The series is this gesture. `holding` is the button of its last step
    /// while that step is still down.
    Yes {
        claimed: EventSet,
        holding: Option<Button>,
    },
}

/// The upper end of a [`Span`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    Unbounded,
    /// The click window of the settings.
    Window,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the span of a hold step (task Tx3pd9)")
    )]
    At(Duration),
}

/// The durations from `min` up to, but not including, `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    min: Duration,
    max: Bound,
}

impl Span {
    const ANY: Self = Self {
        min: Duration::ZERO,
        max: Bound::Unbounded,
    };
    const WITHIN_WINDOW: Self = Self {
        min: Duration::ZERO,
        max: Bound::Window,
    };

    /// Returns the span with `window` as its click window.
    fn resolve(self, window: Duration) -> Limits {
        let max = match self.max {
            Bound::Unbounded => None,
            Bound::Window => Some(window),
            Bound::At(max) => Some(max),
        };
        Limits { min: self.min, max }
    }
}

/// A [`Span`] with its click window resolved. A `max` of None has no end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Limits {
    min: Duration,
    max: Option<Duration>,
}

impl Limits {
    fn contains(self, duration: Duration) -> bool {
        duration >= self.min && self.max.is_none_or(|max| duration < max)
    }
}

/// One step of a pattern (ADR 7, "Gestures"). The first step's `gap` is not
/// read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step<S = Span> {
    button: Button,
    down: S,
    gap: S,
}

impl Step {
    fn tap(button: Button) -> Self {
        Self {
            button,
            down: Span::ANY,
            gap: Span::WITHIN_WINDOW,
        }
    }

    fn resolve(self, window: Duration) -> Step<Limits> {
        Step {
            button: self.button,
            down: self.down.resolve(window),
            gap: self.gap.resolve(window),
        }
    }
}

fn uses<S>(steps: &[Step<S>], button: Button) -> bool {
    steps.iter().any(|step| step.button == button)
}

/// A sequence of steps, such as a double click or a tap of one button and
/// then another (ADR 7, "Gestures").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pattern(Vec<Step>);

impl Pattern {
    pub(crate) fn taps(buttons: impl IntoIterator<Item = Button>) -> Self {
        Self(buttons.into_iter().map(Step::tap).collect())
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_single_tap(&self, button: Button) -> bool {
        self.0 == [Step::tap(button)]
    }

    pub(crate) fn shares_a_button(&self, other: &Self) -> bool {
        other.0.iter().any(|step| uses(&self.0, step.button))
    }
}

/// When a pattern fires (ADR 7, "Gestures").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fires {
    /// Once no press follows the last release within the click window (ADR 7,
    /// "A contested series is decided when it ends").
    WhenSeriesEnds,
    /// Once the last step is down, with the key held until that step's release
    /// (ADR 7, "An uncontested pattern fires at once").
    WhileLastStepDown,
}

/// Where the series stands after the events read so far.
#[derive(Clone, Copy)]
enum Phase<'a> {
    /// No step has been pressed.
    Start,
    Down(Down<'a>),
    Up(Up<'a>),
}

/// `step` was pressed at `at` and is still down, and `rest` follow it.
#[derive(Clone, Copy)]
struct Down<'a> {
    step: &'a Step<Limits>,
    at: Time,
    rest: &'a [Step<Limits>],
}

/// The last step read was released at `at`, and `rest` follow it.
#[derive(Clone, Copy)]
struct Up<'a> {
    at: Time,
    rest: &'a [Step<Limits>],
}

/// The recognizer of one pattern. The same type matches any pattern, and a
/// binding gives the pattern its key (ADR 7, "Gestures").
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Taps {
    steps: Vec<Step<Limits>>,
    window: Duration,
    fires: Fires,
}

impl Taps {
    pub(crate) fn new(pattern: &Pattern, window: Duration, fires: Fires) -> Self {
        Self {
            steps: pattern.0.iter().map(|step| step.resolve(window)).collect(),
            window,
            fires,
        }
    }

    /// Returns how long the series waits after a release for its next step.
    /// After the last step, which is None, it waits one click window.
    fn gap_end(&self, step: Option<&Step<Limits>>) -> Option<Duration> {
        step.map_or(Some(self.window), |step| step.gap.max)
    }

    /// Returns the verdict on `events` at `now`. It depends only on the events
    /// and the time, so the recognizer keeps no state (ADR 7, "Gestures").
    pub(crate) fn check(&self, events: &[Event], now: Time) -> Verdict {
        let mut phase = Phase::Start;
        let mut claimed = EventSet::default();
        for event in events.iter().filter(|e| uses(&self.steps, e.button)) {
            phase = match event.edge {
                Edge::Press => {
                    let (gap, rest) = match phase {
                        // A second button down at once is a chord, and no step is a chord yet (task Tx3pd9).
                        Phase::Down(_) => return Verdict::No,
                        Phase::Start => (None, self.steps.as_slice()),
                        Phase::Up(up) => (Some(event.at.saturating_since(up.at)), up.rest),
                    };
                    // A press after the series ended starts the next one, which is not ours to judge.
                    if gap
                        .zip(self.gap_end(rest.first()))
                        .is_some_and(|(gap, end)| gap >= end)
                    {
                        break;
                    }
                    let Some((step, rest)) = rest.split_first() else {
                        return Verdict::No;
                    };
                    if step.button != event.button || gap.is_some_and(|gap| !step.gap.contains(gap))
                    {
                        return Verdict::No;
                    }
                    Phase::Down(Down {
                        step,
                        at: event.at,
                        rest,
                    })
                }
                Edge::Release => match phase {
                    Phase::Down(down) if down.step.button == event.button => {
                        if !down.step.down.contains(event.at.saturating_since(down.at)) {
                            return Verdict::No;
                        }
                        Phase::Up(Up {
                            at: event.at,
                            rest: down.rest,
                        })
                    }
                    // The release of a press from before the series.
                    _ => continue,
                },
            };
            claimed.insert(event);
        }
        match phase {
            Phase::Start => Verdict::No,
            Phase::Down(down) => self.while_down(down, now, claimed),
            Phase::Up(up) => self.after_release(up, now, claimed),
        }
    }

    /// Returns the verdict while the step of `down` is still down.
    fn while_down(&self, down: Down<'_>, now: Time, claimed: EventSet) -> Verdict {
        let held = now.saturating_since(down.at);
        let end = down.step.down.max;
        if end.is_some_and(|end| held >= end) {
            return Verdict::No;
        }
        let fires_after = (down.rest.is_empty() && self.fires == Fires::WhileLastStepDown)
            .then_some(down.step.down.min);
        if fires_after.is_some_and(|min| held >= min) {
            return Verdict::Yes {
                claimed,
                holding: Some(down.step.button),
            };
        }
        let until = earliest(fires_after, end).map(|span| down.at + span);
        Verdict::Maybe { claimed, until }
    }

    /// Returns the verdict after the series' last release, which `up` holds.
    fn after_release(&self, up: Up<'_>, now: Time, claimed: EventSet) -> Verdict {
        match self.gap_end(up.rest.first()).map(|end| up.at + end) {
            Some(end) if now >= end && up.rest.is_empty() => Verdict::Yes {
                claimed,
                holding: None,
            },
            Some(end) if now >= end => Verdict::No,
            until => Verdict::Maybe { claimed, until },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Usage;
    use crate::device::testing::*;
    use crate::event::EventId;
    use crate::time::testing::{WINDOW, ms};

    /// Returns `events` with ids that are their positions.
    fn numbered<const N: usize>(mut events: [Event; N]) -> [Event; N] {
        for (i, event) in events.iter_mut().enumerate() {
            event.id = EventId::new(u64::try_from(i).unwrap());
        }
        events
    }

    /// Returns the set of the events at `positions`, as [`numbered`] numbers them.
    fn claim(positions: &[u64]) -> EventSet {
        positions.iter().map(|&p| EventId::new(p)).collect()
    }

    fn event(usage: Usage, edge: Edge, at: u64) -> Event {
        Event {
            id: EventId::new(0),
            button: button(usage),
            edge,
            at: ms(at),
        }
    }

    /// Returns the recognizer of `pattern`, as a pattern that shares a button
    /// with another.
    fn contested(pattern: &Pattern) -> Taps {
        Taps::new(pattern, WINDOW, Fires::WhenSeriesEnds)
    }

    /// Returns the recognizer of a pattern of taps of `usages`, as a pattern
    /// that shares a button with another.
    fn taps(usages: &[Usage]) -> Taps {
        contested(&Pattern::taps(usages.iter().map(|&u| button(u))))
    }

    fn maybe(claimed: &[u64], until: Option<u64>) -> Verdict {
        Verdict::Maybe {
            claimed: claim(claimed),
            until: until.map(ms),
        }
    }

    fn yes(claimed: &[u64]) -> Verdict {
        Verdict::Yes {
            claimed: claim(claimed),
            holding: None,
        }
    }

    #[test]
    fn waits_while_held_and_for_the_window() {
        let single = taps(&[VOLUME_UP]);
        let press = numbered([event(VOLUME_UP, Edge::Press, 0)]);
        assert_eq!(single.check(&press, ms(5000)), maybe(&[0], None));
        let click = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_UP, Edge::Release, 100),
        ]);
        assert_eq!(single.check(&click, ms(399)), maybe(&[0, 1], Some(400)));
        assert_eq!(single.check(&click, ms(400)), yes(&[0, 1]));
    }

    #[test]
    fn counts_the_taps() {
        let double = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_UP, Edge::Release, 100),
            event(VOLUME_UP, Edge::Press, 250),
            event(VOLUME_UP, Edge::Release, 350),
        ]);
        assert_eq!(taps(&[VOLUME_UP]).check(&double, ms(350)), Verdict::No);
        assert_eq!(
            taps(&[VOLUME_UP, VOLUME_UP]).check(&double, ms(650)),
            yes(&[0, 1, 2, 3])
        );
        let triple = taps(&[VOLUME_UP, VOLUME_UP, VOLUME_UP]);
        assert_eq!(
            triple.check(&double, ms(649)),
            maybe(&[0, 1, 2, 3], Some(650))
        );
        assert_eq!(triple.check(&double, ms(650)), Verdict::No);
    }

    #[test]
    fn matches_taps_of_different_buttons() {
        let big_then_small = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_UP, Edge::Release, 100),
            event(VOLUME_DOWN, Edge::Press, 200),
            event(VOLUME_DOWN, Edge::Release, 300),
        ]);
        assert_eq!(
            taps(&[VOLUME_UP, VOLUME_DOWN]).check(&big_then_small, ms(600)),
            yes(&[0, 1, 2, 3])
        );
        assert_eq!(
            taps(&[VOLUME_DOWN, VOLUME_UP]).check(&big_then_small, ms(600)),
            Verdict::No
        );
    }

    #[test]
    fn claims_only_its_own_events() {
        let interleaved = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_DOWN, Edge::Press, 10),
            event(VOLUME_UP, Edge::Release, 100),
            event(VOLUME_DOWN, Edge::Release, 110),
        ]);
        assert_eq!(
            taps(&[VOLUME_UP]).check(&interleaved, ms(400)),
            yes(&[0, 2])
        );
        assert_eq!(
            taps(&[VOLUME_DOWN]).check(&interleaved, ms(410)),
            yes(&[1, 3])
        );
    }

    #[test]
    fn a_press_after_the_window_starts_the_next_series() {
        let two_series = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_UP, Edge::Release, 100),
            event(VOLUME_UP, Edge::Press, 400),
        ]);
        assert_eq!(taps(&[VOLUME_UP]).check(&two_series, ms(400)), yes(&[0, 1]));
    }

    #[test]
    fn ignores_buttons_outside_the_pattern() {
        let other = numbered([event(VOLUME_DOWN, Edge::Press, 0)]);
        assert_eq!(taps(&[VOLUME_UP]).check(&other, ms(0)), Verdict::No);
    }

    #[test]
    fn a_step_out_of_its_spans_is_no_match() {
        let quick = Step {
            down: Span {
                min: Duration::ZERO,
                max: Bound::At(Duration::from_millis(50)),
            },
            ..Step::tap(button(VOLUME_UP))
        };
        let late = Step {
            gap: Span {
                min: Duration::from_millis(200),
                max: Bound::Window,
            },
            ..Step::tap(button(VOLUME_UP))
        };
        let pattern = |steps: Vec<Step>| contested(&Pattern(steps));
        let click = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_UP, Edge::Release, 100),
        ]);
        assert_eq!(pattern(vec![quick]).check(&click, ms(400)), Verdict::No);
        let press = numbered([event(VOLUME_UP, Edge::Press, 0)]);
        assert_eq!(
            pattern(vec![quick]).check(&press, ms(10)),
            maybe(&[0], Some(50))
        );
        assert_eq!(pattern(vec![quick]).check(&press, ms(50)), Verdict::No);
        let double = numbered([
            event(VOLUME_UP, Edge::Press, 0),
            event(VOLUME_UP, Edge::Release, 100),
            event(VOLUME_UP, Edge::Press, 150),
            event(VOLUME_UP, Edge::Release, 200),
        ]);
        let tap = Step::tap(button(VOLUME_UP));
        assert_eq!(
            pattern(vec![tap, late]).check(&double, ms(500)),
            Verdict::No
        );
    }

    #[test]
    fn an_uncontested_pattern_fires_while_its_last_step_is_down() {
        let pattern = Pattern::taps([button(VOLUME_DOWN)]);
        let single = Taps::new(&pattern, WINDOW, Fires::WhileLastStepDown);
        let press = numbered([event(VOLUME_DOWN, Edge::Press, 0)]);
        assert_eq!(
            single.check(&press, ms(0)),
            Verdict::Yes {
                claimed: claim(&[0]),
                holding: Some(button(VOLUME_DOWN))
            }
        );
    }
}
