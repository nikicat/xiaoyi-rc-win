use std::fmt;
use std::ops::BitAnd;

/// A usage ID on the HID Consumer page. It is a u32 so that it can hold any
/// value of the registry contract that both drivers share (ADR 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage(pub u32);

impl fmt::UpperHex for Usage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

/// The usage of each report bit, in report order. The registry contract that
/// both drivers share uses the same usages (ADR 6).
#[rustfmt::skip]
pub const USAGES: [Usage; 19] = [
    Usage(0x0B5), Usage(0x0B6), Usage(0x0B7), Usage(0x0B8), Usage(0x0CD), Usage(0x0E2), Usage(0x0E9), Usage(0x0EA), // next, prev, stop, eject, play/pause, mute, vol+, vol-
    Usage(0x183), Usage(0x18A), Usage(0x192), Usage(0x194), // AL media config, mail, calculator, local browser
    Usage(0x221), Usage(0x223), Usage(0x224), Usage(0x225), // AC search, home, back, forward
    Usage(0x226), Usage(0x227), Usage(0x22A), // AC stop, refresh, bookmarks
];

const BUTTONS: usize = USAGES.len();
const PAYLOAD_SIZE: usize = BUTTONS.div_ceil(8);

/// The `Remap<usage>` registry value that makes a button send nothing. It is
/// part of the registry contract that both drivers share (ADR 6).
const USAGE_NONE: u32 = 0;

/// Returns the usage that a `Remap<usage>` registry value tells its button to
/// send, or None when the value is `USAGE_NONE` and the button sends nothing.
pub fn remap_target(value: u32) -> Option<Usage> {
    match value {
        USAGE_NONE => None,
        v => Some(Usage(v)),
    }
}

use button::Button;

/// Keeps `Button`'s index private, so that `Button::ALL` is its only
/// constructor.
mod button {
    use super::{BUTTONS, USAGES, UnknownUsage, Usage};

    /// A button of the remote, by its bit in the report.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct Button(usize);

    impl Button {
        /// Every button, in report order.
        pub(super) const ALL: [Self; BUTTONS] = {
            let mut all = [Self(0); BUTTONS];
            let mut i = 0;
            while i < BUTTONS {
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

        pub(super) fn target(usage: Option<Usage>) -> Result<Option<Self>, UnknownUsage> {
            usage.map(Self::from_usage).transpose()
        }

        pub(super) fn all() -> impl Iterator<Item = Self> {
            Self::ALL.into_iter()
        }

        /// The button's position in report order.
        pub(super) const fn index(self) -> usize {
            self.0
        }

        pub(super) const fn byte(self) -> usize {
            self.0 / 8
        }

        pub(super) const fn mask(self) -> u8 {
            1 << (self.0 % 8)
        }
    }
}

/// The buttons of an input report, followed by padding bits that carry no
/// button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Payload([u8; PAYLOAD_SIZE]);

impl Payload {
    /// Every padding bit set, and no button.
    const PADDING: Self = {
        let mut padding = Self([0xFF; PAYLOAD_SIZE]);
        let mut buttons: &[Button] = &Button::ALL;
        while let [button, rest @ ..] = buttons {
            padding.remove(*button);
            buttons = rest;
        }
        padding
    };

    fn contains(self, button: Button) -> bool {
        self.0[button.byte()] & button.mask() != 0
    }

    fn insert(&mut self, button: Button) {
        self.0[button.byte()] |= button.mask();
    }

    const fn remove(&mut self, button: Button) {
        self.0[button.byte()] &= !button.mask();
    }

    fn any_button(self) -> bool {
        Button::all().any(|b| self.contains(b))
    }
}

impl BitAnd for Payload {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] & rhs.0[i]))
    }
}

/// The remote's only input report (docs/report-map.bin), borrowed from a
/// read-report buffer so that changes to it land in the buffer.
pub struct InputReport<'a>(&'a mut [u8; PAYLOAD_SIZE]);

impl<'a> InputReport<'a> {
    const ID: u8 = 3;
    const SIZE: usize = 1 + PAYLOAD_SIZE;

    /// Returns the input report at the start of `buf`, or None when `buf` holds
    /// another report or is too short.
    pub fn parse(buf: &'a mut [u8]) -> Option<Self> {
        match buf.first_chunk_mut::<{ InputReport::SIZE }>()? {
            [Self::ID, payload @ ..] => Some(Self(payload)),
            _ => None,
        }
    }

    /// Checks whether at least one button is down. The padding bits do not count.
    pub fn is_press(&self) -> bool {
        self.buttons().any_button()
    }

    fn buttons(&self) -> Payload {
        Payload(*self.0)
    }

    fn set_buttons(&mut self, buttons: Payload) {
        *self.0 = buttons.0;
    }
}

/// A usage that the remote's report does not carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownUsage(pub Usage);

/// A remapping of the remote's buttons onto the usages its report carries.
/// The default sends every button's own usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remap {
    /// The button each button's bit moves to, or `None` to send nothing.
    targets: [Option<Button>; BUTTONS],
}

impl Default for Remap {
    fn default() -> Self {
        Self {
            targets: Button::ALL.map(Some),
        }
    }
}

impl Remap {
    /// Makes the button whose own usage is `from` send `to` instead, or send
    /// nothing when `to` is None. Errors with UnknownUsage when either usage is
    /// not in the report, and then leaves the map unchanged.
    pub fn set(&mut self, from: Usage, to: Option<Usage>) -> Result<(), UnknownUsage> {
        let source = Button::from_usage(from)?;
        *self.target_mut(source) = Button::target(to)?;
        Ok(())
    }

    fn target_mut(&mut self, button: Button) -> &mut Option<Button> {
        &mut self.targets[button.index()]
    }

    /// Returns the buttons that the map makes the `pressed` buttons send. The
    /// padding bits of `pressed` stay as they were.
    fn remap(&self, pressed: Payload) -> Payload {
        let mut sent = pressed & Payload::PADDING;
        for (button, target) in Button::all().zip(self.targets) {
            if let Some(target) = target.filter(|_| pressed.contains(button)) {
                sent.insert(target);
            }
        }
        sent
    }

    /// Replaces the buttons of `report` with the ones that `remap` returns for
    /// them.
    pub fn apply(&self, report: &mut InputReport<'_>) {
        report.set_buttons(self.remap(report.buttons()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::BitOr;

    impl BitOr for Payload {
        type Output = Self;

        fn bitor(self, rhs: Self) -> Self {
            Self(std::array::from_fn(|i| self.0[i] | rhs.0[i]))
        }
    }

    /// A Consumer usage the report does not carry.
    const POWER: Usage = Usage(0x030);
    const PLAY_PAUSE: Usage = Usage(0x0CD);
    const VOLUME_UP: Usage = Usage(0x0E9);
    const VOLUME_DOWN: Usage = Usage(0x0EA);
    const BOOKMARKS: Usage = Usage(0x22A);

    fn pressed(usages: &[Usage]) -> Payload {
        let mut buttons = Payload::default();
        for &usage in usages {
            buttons.insert(Button::from_usage(usage).unwrap());
        }
        buttons
    }

    /// A buffer that holds the remote's input report with the given buttons.
    fn report(buttons: Payload) -> [u8; InputReport::SIZE] {
        let mut r = [0; InputReport::SIZE];
        r[0] = InputReport::ID;
        r[1..].copy_from_slice(&buttons.0);
        r
    }

    fn other_id_report(buttons: Payload) -> [u8; InputReport::SIZE] {
        let mut r = report(buttons);
        r[0] = InputReport::ID + 1;
        r
    }

    fn short(report: &mut [u8; InputReport::SIZE]) -> &mut [u8] {
        &mut report[..InputReport::SIZE - 1]
    }

    fn is_press(buttons: Payload) -> bool {
        InputReport::parse(&mut report(buttons)).unwrap().is_press()
    }

    fn vol_up_to_play_pause() -> Remap {
        let mut map = Remap::default();
        map.set(VOLUME_UP, Some(PLAY_PAUSE)).unwrap();
        map
    }

    #[test]
    fn usage_bits() {
        assert_eq!(Button::from_usage(VOLUME_UP), Ok(Button::ALL[6]));
        assert_eq!(Button::from_usage(BOOKMARKS), Ok(Button::ALL[18]));
        assert_eq!(Button::from_usage(POWER), Err(UnknownUsage(POWER)));
    }

    #[test]
    fn decodes_registry_values() {
        assert_eq!(remap_target(USAGE_NONE), None);
        assert_eq!(remap_target(0x0CD), Some(PLAY_PAUSE));
    }

    #[test]
    fn padding_covers_the_bits_past_the_buttons() {
        assert_eq!(Payload::PADDING, Payload([0, 0, 0xF8]));
    }

    #[test]
    fn presses() {
        assert!(!is_press(Payload::default()), "release");
        assert!(is_press(pressed(&[VOLUME_UP])), "vol+");
        assert!(
            is_press(pressed(&[BOOKMARKS])),
            "bookmarks in the last byte"
        );
        assert!(!is_press(Payload::PADDING), "padding alone");
    }

    #[test]
    fn identity_map_leaves_buttons_alone() {
        assert_eq!(
            Remap::default().remap(pressed(&[VOLUME_UP])),
            pressed(&[VOLUME_UP])
        );
    }

    #[test]
    fn remaps() {
        let mut map = vol_up_to_play_pause();
        assert_eq!(map.remap(pressed(&[VOLUME_UP])), pressed(&[PLAY_PAUSE]));
        assert_eq!(map.remap(Payload::default()), Payload::default());
        assert_eq!(
            map.remap(pressed(&[VOLUME_UP, VOLUME_DOWN])),
            pressed(&[PLAY_PAUSE, VOLUME_DOWN])
        );
        assert_eq!(
            map.remap(pressed(&[VOLUME_UP]) | Payload::PADDING),
            pressed(&[PLAY_PAUSE]) | Payload::PADDING
        );

        map.set(VOLUME_DOWN, Some(BOOKMARKS)).unwrap();
        assert_eq!(map.remap(pressed(&[VOLUME_DOWN])), pressed(&[BOOKMARKS]));

        map.set(VOLUME_DOWN, None).unwrap();
        assert_eq!(map.remap(pressed(&[VOLUME_DOWN])), Payload::default());
    }

    #[test]
    fn rejects_unknown_usages() {
        let mut map = vol_up_to_play_pause();
        assert_eq!(map.set(VOLUME_UP, Some(POWER)), Err(UnknownUsage(POWER)));
        assert_eq!(map.set(POWER, Some(VOLUME_UP)), Err(UnknownUsage(POWER)));
        assert_eq!(map.remap(pressed(&[VOLUME_UP])), pressed(&[PLAY_PAUSE]));
    }

    #[test]
    fn applies_to_the_buffer() {
        let mut buf = report(pressed(&[VOLUME_UP]));
        let mut input = InputReport::parse(&mut buf).unwrap();
        vol_up_to_play_pause().apply(&mut input);
        assert_eq!(buf, report(pressed(&[PLAY_PAUSE])));
    }

    #[test]
    fn leaves_other_reports_alone() {
        let vol_up = pressed(&[VOLUME_UP]);
        assert!(
            InputReport::parse(&mut other_id_report(vol_up)).is_none(),
            "other report ID"
        );
        assert!(
            InputReport::parse(short(&mut report(vol_up))).is_none(),
            "short report"
        );
        assert!(InputReport::parse(&mut []).is_none(), "empty report");
    }
}
