//! Every change of a button's state becomes a timed event in one history, and
//! no button has a path of its own (ADR 7, "Layers").

use std::collections::BTreeSet;

use crate::device::{Button, Buttons};
use crate::time::Time;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edge {
    Press,
    Release,
}

impl Edge {
    /// Returns Press for a button that goes down, Release for one that goes up, and
    /// None when `was` and `is` agree.
    pub(crate) fn between(was: bool, is: bool) -> Option<Self> {
        match (was, is) {
            (false, true) => Some(Self::Press),
            (true, false) => Some(Self::Release),
            _ => None,
        }
    }
}

/// The identity of an event, unique among the events of one [`Tracker`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EventId(u64);

#[cfg(test)]
impl EventId {
    pub(crate) const fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Event {
    pub(crate) id: EventId,
    pub(crate) button: Button,
    pub(crate) edge: Edge,
    pub(crate) at: Time,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EventSet(BTreeSet<EventId>);

impl EventSet {
    pub(crate) fn insert(&mut self, event: &Event) {
        self.0.insert(event.id);
    }

    pub(crate) fn add(&mut self, other: &Self) {
        self.0.extend(&other.0);
    }

    pub(crate) fn contains(&self, event: &Event) -> bool {
        self.0.contains(&event.id)
    }

    pub(crate) fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(&other.0)
    }
}

#[cfg(test)]
impl FromIterator<EventId> for EventSet {
    fn from_iter<I: IntoIterator<Item = EventId>>(ids: I) -> Self {
        Self(ids.into_iter().collect())
    }
}

/// Turns the buttons of each input report into the events that changed them.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    /// The buttons of the last input report.
    last: Buttons,
    next_id: u64,
}

impl Tracker {
    /// Records `buttons`, which came at `now`, as the latest state. Returns the
    /// press or release of each button that changed since the previous call.
    pub(crate) fn track(&mut self, buttons: Buttons, now: Time) -> Vec<Event> {
        let last = std::mem::replace(&mut self.last, buttons);
        Button::all()
            .filter_map(|button| {
                let edge = Edge::between(last.is_down(button), buttons.is_down(button))?;
                Some(Event {
                    id: self.new_id(),
                    button,
                    edge,
                    at: now,
                })
            })
            .collect()
    }

    fn new_id(&mut self) -> EventId {
        let id = EventId(self.next_id);
        self.next_id += 1;
        id
    }
}

/// The events that a gesture may still claim, oldest first.
#[derive(Debug, Default)]
pub(crate) struct History(Vec<Event>);

impl History {
    pub(crate) fn push(&mut self, events: impl IntoIterator<Item = Event>) {
        self.0.extend(events);
    }

    pub(crate) fn events(&self) -> &[Event] {
        &self.0
    }

    pub(crate) fn remove(&mut self, events: &EventSet) {
        self.0.retain(|event| !events.contains(event));
    }

    pub(crate) fn keep(&mut self, events: &EventSet) {
        self.0.retain(|event| events.contains(event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::testing::*;
    use crate::time::testing::ms;

    #[test]
    fn tracks_presses_and_releases() {
        let mut tracker = Tracker::default();
        let now = ms(5);
        let big = button(VOLUME_UP);
        let small = button(VOLUME_DOWN);
        let event = |id, button, edge| Event {
            id: EventId(id),
            button,
            edge,
            at: now,
        };
        assert_eq!(
            tracker.track(buttons(&[VOLUME_UP]), now),
            [event(0, big, Edge::Press)]
        );
        assert_eq!(tracker.track(buttons(&[VOLUME_UP]), now), []);
        assert_eq!(
            tracker.track(buttons(&[VOLUME_DOWN]), now),
            [event(1, big, Edge::Release), event(2, small, Edge::Press)]
        );
    }

    #[test]
    fn removes_and_keeps_claimed_events() {
        let mut tracker = Tracker::default();
        let mut history = History::default();
        history.push(tracker.track(buttons(&[VOLUME_UP]), ms(0)));
        history.push(tracker.track(buttons(&[]), ms(1)));
        history.push(tracker.track(buttons(&[VOLUME_DOWN]), ms(2)));
        let ids = |events: &[Event]| -> EventSet { events.iter().map(|e| e.id).collect() };
        let second = ids(&history.events()[1..2]);
        history.remove(&second);
        assert_eq!(
            history.events().iter().map(|e| e.at).collect::<Vec<_>>(),
            [ms(0), ms(2)]
        );
        let last = ids(&history.events()[1..]);
        history.keep(&last);
        assert_eq!(
            history.events().iter().map(|e| e.at).collect::<Vec<_>>(),
            [ms(2)]
        );
    }
}
