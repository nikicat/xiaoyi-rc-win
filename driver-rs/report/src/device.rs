//! The only layer that knows the remote's report. The layers above see buttons
//! and keys, so that another device is another description of its reports
//! (ADR 7, "Layers").

/// A usage ID on the HID Consumer page. It can be a usage that the remote's
/// report does not carry, such as one that a `Gestures` line names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage(pub(crate) u32);

/// The usage of each report bit, in report order (`docs/report-map.bin`).
#[rustfmt::skip]
pub(crate) const USAGES: [Usage; 19] = [
    Usage(0x0B5), Usage(0x0B6), Usage(0x0B7), Usage(0x0B8), Usage(0x0CD), Usage(0x0E2), Usage(0x0E9), Usage(0x0EA), // next, prev, stop, eject, play/pause, mute, vol+, vol-
    Usage(0x183), Usage(0x18A), Usage(0x192), Usage(0x194), // AL media config, mail, calculator, local browser
    Usage(0x221), Usage(0x223), Usage(0x224), Usage(0x225), // AC search, home, back, forward
    Usage(0x226), Usage(0x227), Usage(0x22A), // AC stop, refresh, bookmarks
];

const BITS: usize = USAGES.len();
const PAYLOAD_SIZE: usize = BITS.div_ceil(8);

use bit::Bit;

/// Keeps a bit's index private, so that `Bit::ALL` is its only constructor.
mod bit {
    use super::{BITS, USAGES, UnknownUsage, Usage};

    /// A bit of the input report, which carries one usage.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Bit(usize);

    impl Bit {
        /// Every bit, in report order.
        pub(super) const ALL: [Self; BITS] = {
            let mut all = [Self(0); BITS];
            let mut i = 0;
            while i < BITS {
                all[i] = Self(i);
                i += 1;
            }
            all
        };

        pub(super) fn from_usage(usage: Usage) -> Result<Self, UnknownUsage> {
            USAGES
                .iter()
                .position(|&u| u == usage)
                .map(|i| Self::ALL[i])
                .ok_or(UnknownUsage(usage))
        }

        pub(super) const fn byte(self) -> usize {
            self.0 / 8
        }

        pub(super) const fn mask(self) -> u8 {
            1 << (self.0 % 8)
        }
    }
}

/// An input of the remote (ADR 7, "Layers"). It is named by its own usage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Button(Bit);

impl Button {
    pub(crate) fn from_usage(usage: Usage) -> Result<Self, UnknownUsage> {
        Bit::from_usage(usage).map(Self)
    }

    /// Returns every button, in report order.
    pub(crate) fn all() -> impl Iterator<Item = Self> {
        Bit::ALL.into_iter().map(Self)
    }

    /// Returns the key of the button's own usage.
    pub(crate) const fn own_key(self) -> Key {
        Key(self.0)
    }
}

/// A usage that the filter reports to Windows (ADR 7, "Layers").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Key(Bit);

impl Key {
    pub(crate) fn from_usage(usage: Usage) -> Result<Self, UnknownUsage> {
        Bit::from_usage(usage).map(Self)
    }
}

/// The bits of a report after its ID: one per usage, then padding bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Bits([u8; PAYLOAD_SIZE]);

impl Bits {
    fn get(self, bit: Bit) -> bool {
        self.0[bit.byte()] & bit.mask() != 0
    }

    fn set(&mut self, bit: Bit) {
        self.0[bit.byte()] |= bit.mask();
    }

    fn clear(&mut self, bit: Bit) {
        self.0[bit.byte()] &= !bit.mask();
    }

    /// Returns the usage bits that are set, with every padding bit clear.
    fn usages(self) -> Self {
        let mut usages = Self::default();
        Bit::ALL
            .into_iter()
            .filter(|&bit| self.get(bit))
            .for_each(|bit| usages.set(bit));
        usages
    }
}

/// The buttons that are down in an input report from the remote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Buttons(Bits);

impl Buttons {
    pub(crate) fn is_down(self, button: Button) -> bool {
        self.0.get(button.0)
    }
}

/// The keys that a report for Windows holds down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Keys(Bits);

impl Keys {
    /// Returns these keys with `key` held as well.
    pub(crate) fn with(mut self, key: Key) -> Self {
        self.0.set(key.0);
        self
    }

    pub(crate) fn without(mut self, key: Key) -> Self {
        self.0.clear(key.0);
        self
    }

    /// Returns the report that carries these keys to Windows.
    pub(crate) fn to_report(self) -> KeyReport {
        let mut report = [0; InputReport::SIZE];
        report[0] = InputReport::ID;
        report[1..].copy_from_slice(&self.0.0);
        KeyReport(report)
    }
}

/// A report to Windows of the keys that are held down. It has the layout of
/// the remote's input report (ADR 7, "Layers").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyReport([u8; InputReport::SIZE]);

impl KeyReport {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// The remote's only input report (`docs/report-map.bin`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputReport(Bits);

impl InputReport {
    const ID: u8 = 3;
    /// The report's length in bytes, with its report ID.
    pub(crate) const SIZE: usize = 1 + PAYLOAD_SIZE;

    /// Returns the input report at the start of `buf`, or None when `buf` holds
    /// another report or is too short.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        match buf.first_chunk::<{ InputReport::SIZE }>()? {
            [Self::ID, payload @ ..] => Some(Self(Bits(*payload))),
            _ => None,
        }
    }

    pub(crate) fn buttons(&self) -> Buttons {
        Buttons(self.0.usages())
    }
}

/// A usage that the remote's report does not carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnknownUsage(pub(crate) Usage);

/// Named usages, buttons, keys and reports for the crate's tests.
#[cfg(test)]
pub(crate) mod testing {
    use super::{Button, Buttons, InputReport, Key, KeyReport, Keys, Usage};

    pub(crate) const NEXT: Usage = Usage(0x0B5);
    pub(crate) const PREVIOUS: Usage = Usage(0x0B6);
    pub(crate) const PLAY_PAUSE: Usage = Usage(0x0CD);
    pub(crate) const MUTE: Usage = Usage(0x0E2);
    pub(crate) const VOLUME_UP: Usage = Usage(0x0E9);
    pub(crate) const VOLUME_DOWN: Usage = Usage(0x0EA);
    pub(crate) const BOOKMARKS: Usage = Usage(0x22A);
    /// A Consumer usage the report does not carry.
    pub(crate) const POWER: Usage = Usage(0x030);

    pub(crate) fn button(usage: Usage) -> Button {
        Button::from_usage(usage).unwrap()
    }

    pub(crate) fn key(usage: Usage) -> Key {
        Key::from_usage(usage).unwrap()
    }

    /// Returns `usages` held down.
    pub(crate) fn keys(usages: &[Usage]) -> Keys {
        usages
            .iter()
            .map(|&u| key(u))
            .fold(Keys::default(), Keys::with)
    }

    /// Returns a report with `usages` down.
    pub(crate) fn report(usages: &[Usage]) -> KeyReport {
        keys(usages).to_report()
    }

    /// Returns the remote's input report with `usages` down.
    pub(crate) fn input(usages: &[Usage]) -> InputReport {
        InputReport::parse(report(usages).as_bytes()).unwrap()
    }

    /// Returns the remote's input report with `usages` down and every padding
    /// bit set.
    pub(crate) fn padded_input(usages: &[Usage]) -> InputReport {
        let mut buf = report(usages).as_bytes().to_vec();
        buf[InputReport::SIZE - 1] |= 0xF8;
        InputReport::parse(&buf).unwrap()
    }

    /// Returns the buttons of the input report with `usages` down.
    pub(crate) fn buttons(usages: &[Usage]) -> Buttons {
        input(usages).buttons()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn usage_bits() {
        assert_eq!(Bit::from_usage(VOLUME_UP), Ok(Bit::ALL[6]));
        assert_eq!(Bit::from_usage(BOOKMARKS), Ok(Bit::ALL[18]));
        assert_eq!(Bit::from_usage(POWER), Err(UnknownUsage(POWER)));
    }

    #[test]
    fn parses_the_buttons_and_ignores_the_padding() {
        let buttons = padded_input(&[VOLUME_UP]).buttons();
        assert!(buttons.is_down(button(VOLUME_UP)));
        assert!(!buttons.is_down(button(VOLUME_DOWN)));
        assert_eq!(buttons, input(&[VOLUME_UP]).buttons());
    }

    #[test]
    fn leaves_other_reports_alone() {
        let mut other = report(&[VOLUME_UP]).as_bytes().to_vec();
        other[0] = InputReport::ID + 1;
        assert!(InputReport::parse(&other).is_none(), "other report ID");
        let report = report(&[VOLUME_UP]);
        assert!(
            InputReport::parse(&report.as_bytes()[..InputReport::SIZE - 1]).is_none(),
            "short report"
        );
        assert!(InputReport::parse(&[]).is_none(), "empty report");
    }
}
