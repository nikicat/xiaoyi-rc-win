//! The arbiter, which decides which gestures fire (ADR 7, "Gestures").

use crate::bindings::Gestures;
use crate::device::{Button, Key};
use crate::event::{Event, EventSet, History};
use crate::gesture::Verdict;
use crate::time::{Time, earliest};

/// What the output does with the key of a gesture that fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Firing {
    /// The key goes down and up at once (ADR 7, "Taps are synthesized").
    Tap(Key),
    Press(Press),
}

/// A key that goes down and stays down until `holding` is released (ADR 7, "An
/// uncontested pattern fires at once").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Press {
    pub(crate) key: Key,
    pub(crate) holding: Button,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) firings: Vec<Firing>,
    /// The earliest time when a gesture wants to be asked again, or None when
    /// only the next event can change the answers (ADR 7, "Gestures").
    pub(crate) deadline: Option<Time>,
}

/// Returns what `gestures` decide at `now` (ADR 7, "Gestures"). Removes from
/// `history` the events of the gestures that fire, and the events that none
/// claims once their series has ended.
pub(crate) fn decide(gestures: &Gestures, history: &mut History, now: Time) -> Decision {
    let mut firings = Vec::new();
    loop {
        match ask(gestures, history.events(), now) {
            Round::Fired { claimed, firing } => {
                history.remove(&claimed);
                firings.extend(firing);
            }
            Round::Settled(claims) => {
                history.keep(&claims.taken, now, gestures.window());
                let deadline = claims.deadline;
                return Decision { firings, deadline };
            }
        }
    }
}

/// What one round of asking the gestures comes to.
enum Round {
    /// A gesture fired with the events of `claimed`. Its firing is None when
    /// its binding reports no key.
    Fired {
        claimed: EventSet,
        firing: Option<Firing>,
    },
    /// No gesture can fire yet.
    Settled(Claims),
}

/// The events that the gestures asked so far claim, and their earliest deadline.
#[derive(Default)]
struct Claims {
    taken: EventSet,
    deadline: Option<Time>,
}

impl Claims {
    fn take(&mut self, claimed: &EventSet, until: Option<Time>) {
        self.taken.add(claimed);
        self.deadline = earliest(self.deadline, until);
    }
}

fn ask(gestures: &Gestures, events: &[Event], now: Time) -> Round {
    let mut claims = Claims::default();
    for contender in gestures.iter() {
        match contender.taps.check(events, now) {
            Verdict::No => {}
            Verdict::Yes { claimed, holding } if claims.taken.is_disjoint(&claimed) => {
                let firing = contender.key.map(|key| match holding {
                    Some(holding) => Firing::Press(Press { key, holding }),
                    None => Firing::Tap(key),
                });
                return Round::Fired { claimed, firing };
            }
            Verdict::Yes { claimed, .. } => claims.take(&claimed, None),
            Verdict::Maybe { claimed, until } => claims.take(&claimed, until),
        }
    }
    Round::Settled(claims)
}
