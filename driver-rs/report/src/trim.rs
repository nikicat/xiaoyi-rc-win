const END_COLLECTION: u8 = 0xC0;

/// Returns `desc` without its trailing zero bytes, or None when it has none or
/// what remains does not end in End Collection. The zeros are safe to drop
/// because a valid descriptor ends in End Collection (ADR 2).
pub fn trim_descriptor(desc: &[u8]) -> Option<&[u8]> {
    let end = desc.iter().rposition(|&b| b != 0x00).map_or(0, |i| i + 1);
    match desc[..end].last() {
        Some(&END_COLLECTION) if end < desc.len() => Some(&desc[..end]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::trim_descriptor;

    const REPORT_MAP: &[u8] = include_bytes!("../../../docs/report-map.bin");

    #[test]
    fn real_report_map_trims_to_75() {
        assert_eq!(REPORT_MAP.len(), 122);
        assert_eq!(trim_descriptor(REPORT_MAP), Some(&REPORT_MAP[..75]));
    }

    #[test]
    fn already_trimmed_map_is_not_trimmed() {
        assert_eq!(trim_descriptor(&REPORT_MAP[..75]), None);
    }

    #[test]
    fn all_zeros_are_not_trimmed() {
        assert_eq!(trim_descriptor(&[0; 16]), None);
    }

    #[test]
    fn padding_without_end_collection_is_not_trimmed() {
        let no_end = [0x05, 0x0c, 0x09, 0x01, 0x00, 0x00];
        assert_eq!(trim_descriptor(&no_end), None);
    }

    #[test]
    fn empty_input_is_not_trimmed() {
        assert_eq!(trim_descriptor(&[]), None);
    }
}
