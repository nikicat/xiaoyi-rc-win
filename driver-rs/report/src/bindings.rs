//! The settings of the gestures. Every binding, single taps included, is one
//! line of the `Gestures` registry value (ADR 7, "Settings").

use std::cmp::Reverse;
use std::time::Duration;

use crate::device::{Button, Key, UnknownUsage, Usage};
use crate::gesture::{Fires, Pattern, Taps};

/// The key of a `Gestures` line that turns a button's single tap off (ADR 7,
/// "Only a single tap can report nothing").
const USAGE_NONE: u32 = 0;

/// Returns the usage that the key of a `Gestures` line names, or None when the
/// key is `USAGE_NONE`.
fn key_value(value: u32) -> Option<Usage> {
    match value {
        USAGE_NONE => None,
        v => Some(Usage(v)),
    }
}

/// Why a line of the `Gestures` registry value is rejected (ADR 7, "Settings").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineError {
    /// The line is not `<usage> <usage> ... = <usage>`, in hex.
    Syntax,
    UnknownUsage(Usage),
    /// The pattern has more than one tap and names no key (ADR 7, "Only a
    /// single tap can report nothing").
    NoKey,
}

impl From<UnknownUsage> for LineError {
    fn from(unknown: UnknownUsage) -> Self {
        Self::UnknownUsage(unknown.0)
    }
}

/// A pattern and the key that it reports, or None for no key, as one line of
/// the `Gestures` registry value states them (ADR 7, "Settings").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pattern: Pattern,
    key: Option<Key>,
}

impl Binding {
    /// Returns the binding of the taps of `pattern` to `key`. Errors with
    /// UnknownUsage when the report does not carry a usage, and with NoKey when a
    /// pattern of more than one tap has no key (ADR 7).
    pub(crate) fn new(pattern: &[Usage], key: Option<Usage>) -> Result<Self, LineError> {
        if key.is_none() && pattern.len() > 1 {
            return Err(LineError::NoKey);
        }
        let buttons = pattern
            .iter()
            .map(|&usage| Button::from_usage(usage))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            pattern: Pattern::taps(buttons),
            key: key.map(Key::from_usage).transpose()?,
        })
    }

    /// Returns the binding that a line of the `Gestures` registry value states.
    /// Errors as [`Bindings::set_line`] says.
    fn parse(line: &str) -> Result<Self, LineError> {
        let (pattern, key) = line.split_once('=').ok_or(LineError::Syntax)?;
        let usage = |hex: &str| u32::from_str_radix(hex, 16).map_err(|_| LineError::Syntax);
        let pattern = pattern
            .split_whitespace()
            .map(|step| usage(step).map(Usage))
            .collect::<Result<Vec<_>, _>>()?;
        if pattern.is_empty() {
            return Err(LineError::Syntax);
        }
        Self::new(&pattern, key_value(usage(key.trim())?))
    }

    /// Returns WhenSeriesEnds for a pattern that shares a button with another of
    /// `among`, and WhileLastStepDown otherwise (ADR 7, "Gestures").
    fn fires(&self, among: &[Self]) -> Fires {
        let contested = among.iter().any(|other| {
            other.pattern != self.pattern && self.pattern.shares_a_button(&other.pattern)
        });
        if contested {
            Fires::WhenSeriesEnds
        } else {
            Fires::WhileLastStepDown
        }
    }
}

/// The binding of each pattern. A button whose single tap no binding names
/// reports its own key when tapped once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings(Vec<Binding>);

impl Bindings {
    /// Sets `binding`, in place of any binding of the same pattern.
    pub(crate) fn set(&mut self, binding: Binding) {
        self.0.retain(|b| b.pattern != binding.pattern);
        self.0.push(binding);
    }

    /// Sets the binding that a `Gestures` line states, such as `E9 E9 = E2`, in
    /// place of any binding of the same pattern (ADR 7, "Settings"). Errors with
    /// LineError when the line is rejected, and then changes nothing.
    pub fn set_line(&mut self, line: &str) -> Result<(), LineError> {
        self.set(Binding::parse(line)?);
        Ok(())
    }

    /// Returns the bindings that the lines of a `Gestures` value state. A rejected
    /// line is ignored, and a later line of a pattern replaces an earlier one
    /// (ADR 7, "Settings").
    pub fn from_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Self {
        let mut bindings = Self::default();
        for line in lines {
            let _ = bindings.set_line(line);
        }
        bindings
    }
}

/// A gesture and the key that it reports when it fires, or None for no key.
pub(crate) struct Contender {
    pub(crate) taps: Taps,
    pub(crate) key: Option<Key>,
}

/// The gestures of the settings, in priority order, and the click window that
/// ends their series (ADR 7, "Gestures").
pub(crate) struct Gestures {
    contenders: Vec<Contender>,
    window: Duration,
}

impl Gestures {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Contender> {
        self.contenders.iter()
    }

    pub(crate) fn window(&self) -> Duration {
        self.window
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub bindings: Bindings,
    /// The click window, from each release to the next press (ADR 7, "The click
    /// window").
    pub window: Duration,
}

impl Config {
    /// The default click window (ADR 7, "The click window").
    pub const WINDOW: Duration = Duration::from_millis(200);

    /// Returns every gesture of the settings, the single tap of each button
    /// included, in priority order (ADR 7, "Gestures").
    pub(crate) fn gestures(&self) -> Gestures {
        let Bindings(bound) = &self.bindings;
        let singles = Button::all()
            .filter(|&button| !bound.iter().any(|b| b.pattern.is_single_tap(button)))
            .map(|button| Binding {
                pattern: Pattern::taps([button]),
                key: Some(button.own_key()),
            });
        let mut all: Vec<_> = bound.iter().cloned().chain(singles).collect();
        all.sort_by_key(|binding| Reverse(binding.pattern.len()));
        let contender = |binding: &Binding| Contender {
            taps: Taps::new(&binding.pattern, self.window, binding.fires(&all)),
            key: binding.key,
        };
        Gestures {
            contenders: all.iter().map(contender).collect(),
            window: self.window,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bindings: Bindings::default(),
            window: Self::WINDOW,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::USAGES;
    use crate::device::testing::*;

    fn bind(pattern: &[Usage], key: Option<Usage>) -> Binding {
        Binding::new(pattern, key).unwrap()
    }

    /// Returns the key that a single tap of `usage` reports, where no other
    /// pattern shares its button.
    fn single(bindings: &Bindings, usage: Usage) -> Option<Key> {
        let config = Config {
            bindings: bindings.clone(),
            ..Config::default()
        };
        let pattern = Pattern::taps([button(usage)]);
        let tap = Taps::new(&pattern, config.window, Fires::WhileLastStepDown);
        let gestures = config.gestures();
        gestures.iter().find(|c| c.taps == tap).unwrap().key
    }

    #[test]
    fn decodes_key_values() {
        assert_eq!(key_value(USAGE_NONE), None);
        assert_eq!(key_value(0x0CD), Some(PLAY_PAUSE));
    }

    #[test]
    fn ranks_longer_patterns_first() {
        let mut bindings = Bindings::default();
        bindings.set(bind(&[VOLUME_UP, VOLUME_UP], Some(NEXT)));
        bindings.set(bind(&[VOLUME_UP, VOLUME_UP, VOLUME_UP], Some(PREVIOUS)));
        let config = Config {
            bindings,
            ..Config::default()
        };
        let gestures = config.gestures();
        let keys: Vec<_> = gestures.iter().map(|c| c.key).take(2).collect();
        assert_eq!(keys, [Some(key(PREVIOUS)), Some(key(NEXT))]);
        assert_eq!(
            gestures.iter().count(),
            2 + USAGES.len(),
            "and every button's single tap"
        );
    }

    #[test]
    fn a_single_tap_has_the_buttons_own_key_until_bound() {
        let mut bindings = Bindings::default();
        assert_eq!(single(&bindings, VOLUME_UP), Some(key(VOLUME_UP)));
        bindings.set(bind(&[VOLUME_UP], Some(PLAY_PAUSE)));
        assert_eq!(single(&bindings, VOLUME_UP), Some(key(PLAY_PAUSE)));
        bindings.set(bind(&[VOLUME_UP], None));
        assert_eq!(single(&bindings, VOLUME_UP), None);
    }

    #[test]
    fn parses_registry_lines() {
        let mut bindings = Bindings::default();
        bindings.set_line("E9 E9 = E2").unwrap();
        bindings.set_line("EA = 0").unwrap();
        bindings.set_line(" E9 E9 E9=CD ").unwrap();
        let mut expected = Bindings::default();
        expected.set(bind(&[VOLUME_UP, VOLUME_UP], Some(MUTE)));
        expected.set(bind(&[VOLUME_DOWN], None));
        expected.set(bind(&[VOLUME_UP, VOLUME_UP, VOLUME_UP], Some(PLAY_PAUSE)));
        assert_eq!(bindings, expected);
    }

    fn error_name(error: LineError) -> &'static str {
        match error {
            LineError::Syntax => "Syntax",
            LineError::UnknownUsage(_) => "UnknownUsage",
            LineError::NoKey => "NoKey",
        }
    }

    /// Returns the cases of a corpus file, without its comments and blank lines.
    fn corpus_cases(corpus: &str) -> impl Iterator<Item = &str> {
        corpus
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
    }

    #[test]
    fn reads_the_corpus_of_lines() {
        for case in corpus_cases(include_str!("../../../tests/gesture-lines.txt")) {
            let (line, reading) = case.rsplit_once('\t').expect("a tab before the reading");
            let mut bindings = Bindings::default();
            match bindings.set_line(line) {
                Err(error) => {
                    assert_eq!(error_name(error), reading, "{line:?}");
                    assert_eq!(bindings, Bindings::default(), "{line:?}");
                }
                Ok(()) => {
                    let mut expected = Bindings::default();
                    expected.set_line(reading).expect(reading);
                    assert_eq!(bindings, expected, "{line:?}");
                }
            }
        }
    }

    #[test]
    fn keeps_the_corpus_values_last_accepted_lines() {
        let lines = |side: &'static str| side.split(" | ").filter(|l| !l.is_empty());
        for case in corpus_cases(include_str!("../../../tests/gesture-values.txt")) {
            let (value, kept) = case.split_once('\t').expect("a tab before the kept lines");
            assert_eq!(
                Bindings::from_lines(lines(value)),
                Bindings::from_lines(lines(kept)),
                "{value:?}"
            );
        }
    }

    #[test]
    fn a_new_binding_replaces_the_old() {
        let mut bindings = Bindings::default();
        bindings.set(bind(&[VOLUME_UP, VOLUME_UP], Some(NEXT)));
        bindings.set(bind(&[VOLUME_UP, VOLUME_UP], Some(MUTE)));
        let mut expected = Bindings::default();
        expected.set(bind(&[VOLUME_UP, VOLUME_UP], Some(MUTE)));
        assert_eq!(bindings, expected);
    }
}
