//! The output layer: the reports that Windows gets, from the gestures that
//! fired (ADR 7, "Layers").

use std::collections::VecDeque;

use crate::arbiter::{Firing, Press};
use crate::device::{KeyReport, Keys};
use crate::event::{Edge, Event};

#[derive(Debug, Default)]
pub(crate) struct Output {
    /// The presses of fired gestures that are still held.
    pressed: Vec<Press>,
    last_queued: Keys,
    queue: VecDeque<Keys>,
}

impl Output {
    /// Releases the held keys whose button the events release, and queues a report
    /// when the held keys change.
    pub(crate) fn input(&mut self, events: &[Event]) {
        let released = |button| {
            events
                .iter()
                .any(|e| e.button == button && e.edge == Edge::Release)
        };
        self.pressed.retain(|press| !released(press.holding));
        self.sync();
    }

    /// Queues the reports of a gesture that fired. A tap of a key that is
    /// held already releases it first, so that Windows sees the tap.
    pub(crate) fn fire(&mut self, firing: Firing) {
        match firing {
            Firing::Tap(key) => {
                let held = self.keys();
                for keys in [held.without(key), held.with(key), held] {
                    self.queue_changed(keys);
                }
            }
            Firing::Press(press) => {
                self.pressed.push(press);
                self.sync();
            }
        }
    }

    pub(crate) fn pop(&mut self) -> Option<KeyReport> {
        self.queue.pop_front().map(Keys::to_report)
    }

    pub(crate) fn queued(&self) -> usize {
        self.queue.len()
    }

    pub(crate) fn held_report(&self) -> KeyReport {
        self.last_queued.to_report()
    }

    /// Returns the keys that Windows should hold down now.
    fn keys(&self) -> Keys {
        self.pressed
            .iter()
            .fold(Keys::default(), |keys, press| keys.with(press.key))
    }

    fn sync(&mut self) {
        self.queue_changed(self.keys());
    }

    fn queue_changed(&mut self, keys: Keys) {
        if keys != self.last_queued {
            self.last_queued = keys;
            self.queue.push_back(keys);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::testing::*;
    use crate::event::EventId;
    use crate::time::testing::ms;

    #[test]
    fn a_press_holds_its_key_until_the_release_and_a_tap_keeps_it() {
        let mut output = Output::default();
        output.fire(Firing::Press(Press {
            key: key(MUTE),
            holding: button(VOLUME_DOWN),
        }));
        output.fire(Firing::Tap(key(NEXT)));
        let release = Event {
            id: EventId::new(0),
            button: button(VOLUME_DOWN),
            edge: Edge::Release,
            at: ms(0),
        };
        output.input(&[release]);
        let sent: Vec<_> = std::iter::from_fn(|| output.pop()).collect();
        assert_eq!(
            sent,
            [
                report(&[MUTE]),
                report(&[MUTE, NEXT]),
                report(&[MUTE]),
                report(&[])
            ]
        );
        assert_eq!(output.held_report(), report(&[]));
    }

    #[test]
    fn a_tap_presses_and_releases_its_key() {
        let mut output = Output::default();
        output.fire(Firing::Tap(key(NEXT)));
        let sent: Vec<_> = std::iter::from_fn(|| output.pop()).collect();
        assert_eq!(sent, [report(&[NEXT]), report(&[])]);
    }

    #[test]
    fn a_tap_of_a_held_key_releases_and_presses_it_again() {
        let mut output = Output::default();
        output.fire(Firing::Press(Press {
            key: key(MUTE),
            holding: button(VOLUME_DOWN),
        }));
        output.fire(Firing::Tap(key(MUTE)));
        let sent: Vec<_> = std::iter::from_fn(|| output.pop()).collect();
        assert_eq!(sent, [report(&[MUTE]), report(&[]), report(&[MUTE])]);
        assert_eq!(output.held_report(), report(&[MUTE]));
    }
}
