//! The trim, on any HID descriptor and report descriptor. The input is the
//! HID descriptor, then the report descriptor.

#![no_main]

use libfuzzer_sys::fuzz_target;
use xiaoyi_rc_report::{HidDescriptor, END_COLLECTION};

fuzz_target!(|data: &[u8]| {
    let (hid, report) = data.split_at(data.len().min(HidDescriptor::SIZE));
    let mut hid = hid.to_vec();
    let Some(mut desc) = HidDescriptor::parse(&mut hid) else {
        return;
    };
    let stated = desc.report_length();
    match desc.trim(report) {
        Some(trimmed) => {
            assert!(trimmed.len() < report.len());
            assert!(report.starts_with(trimmed));
            assert!(report[trimmed.len()..].iter().all(|&b| b == 0));
            assert_eq!(trimmed.last(), Some(&END_COLLECTION));
            assert_eq!(usize::from(desc.report_length()), trimmed.len());
        }
        None => assert_eq!(desc.report_length(), stated),
    }
});
