use crate::trim::trim_descriptor;

/// The HID descriptor that IOCTL_HID_GET_DEVICE_DESCRIPTOR returns (HID 1.11,
/// section 6.2.1). It covers only the first nine bytes, which end with the
/// length of the first report descriptor that it lists.
pub struct HidDescriptor<'a>(&'a mut [u8; HidDescriptor::SIZE]);

impl<'a> HidDescriptor<'a> {
    const SIZE: usize = 9;
    const NUM_DESCRIPTORS_OFFSET: usize = 5;

    /// Returns the HID descriptor at the start of `buf`. Returns None when `buf`
    /// is shorter than a HID descriptor, or when the HID descriptor lists no
    /// report descriptor or states a length of 0 for it.
    pub fn parse(buf: &'a mut [u8]) -> Option<Self> {
        let desc = Self(buf.first_chunk_mut()?);
        (desc.0[Self::NUM_DESCRIPTORS_OFFSET] >= 1 && desc.report_length() != 0).then_some(desc)
    }

    /// Returns the length in bytes of the first report descriptor that this HID
    /// descriptor lists.
    pub fn report_length(&self) -> u16 {
        let [.., lo, hi] = *self.0;
        u16::from_le_bytes([lo, hi])
    }

    fn set_report_length(&mut self, length: u16) {
        let [.., lo, hi] = &mut *self.0;
        [*lo, *hi] = length.to_le_bytes();
    }

    /// Drops the trailing zeros of `report`, and makes this HID descriptor state
    /// the trimmed length, so that hidclass asks for no more (ADR 2). Returns None
    /// and changes nothing when `report` is not the stated length or has nothing to trim.
    pub fn trim<'r>(&mut self, report: &'r [u8]) -> Option<&'r [u8]> {
        if report.len() != usize::from(self.report_length()) {
            return None;
        }
        let trimmed = trim_descriptor(report)?;
        self.set_report_length(u16::try_from(trimmed.len()).ok()?);
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::HidDescriptor;

    const REPORT_MAP: &[u8] = include_bytes!("../../../docs/report-map.bin");

    /// A HID descriptor for one report descriptor of `length` bytes.
    fn hid_descriptor(length: u16) -> [u8; HidDescriptor::SIZE] {
        let [lo, hi] = length.to_le_bytes();
        [0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, lo, hi]
    }

    #[test]
    fn trim_states_the_trimmed_length() {
        let mut buf = hid_descriptor(122);
        let mut desc = HidDescriptor::parse(&mut buf).unwrap();
        assert_eq!(desc.trim(REPORT_MAP), Some(&REPORT_MAP[..75]));
        assert_eq!(desc.report_length(), 75);
        assert_eq!(buf, hid_descriptor(75));
    }

    #[test]
    fn trimmed_map_keeps_its_length() {
        let mut buf = hid_descriptor(75);
        let mut desc = HidDescriptor::parse(&mut buf).unwrap();
        assert_eq!(desc.trim(&REPORT_MAP[..75]), None);
        assert_eq!(buf, hid_descriptor(75));
    }

    #[test]
    fn short_read_is_not_trimmed() {
        let mut buf = hid_descriptor(122);
        let mut desc = HidDescriptor::parse(&mut buf).unwrap();
        assert_eq!(desc.trim(&REPORT_MAP[..100]), None);
        assert_eq!(buf, hid_descriptor(122));
    }

    #[test]
    fn longer_buffer_is_parsed() {
        let mut buf = [0xAA; 16];
        buf[..HidDescriptor::SIZE].copy_from_slice(&hid_descriptor(122));
        assert_eq!(
            HidDescriptor::parse(&mut buf).map(|d| d.report_length()),
            Some(122)
        );
    }

    #[test]
    fn short_buffer_is_rejected() {
        assert!(
            HidDescriptor::parse(&mut hid_descriptor(122)[..HidDescriptor::SIZE - 1]).is_none()
        );
    }

    #[test]
    fn no_report_descriptor_is_rejected() {
        let mut buf = hid_descriptor(122);
        buf[HidDescriptor::NUM_DESCRIPTORS_OFFSET] = 0;
        assert!(HidDescriptor::parse(&mut buf).is_none());
    }

    #[test]
    fn empty_report_descriptor_is_rejected() {
        assert!(HidDescriptor::parse(&mut hid_descriptor(0)).is_none());
    }
}
