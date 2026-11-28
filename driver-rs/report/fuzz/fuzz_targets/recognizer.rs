//! The `Recognizer`, on any settings, reports and times.
//!
//! The input is a `Gestures` value, its lines split by `\n`, then a NUL, then
//! the click window in milliseconds as a little-endian u16, then steps of 6
//! bytes: the milliseconds since the last step as a little-endian u16, and 4
//! bytes. When the 4 bytes are an input report, such as `03 40 00 00` (ADR 2),
//! the step is that report; otherwise it is the window timer firing.

#![no_main]

use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use xiaoyi_rc_report::{Bindings, Config, InputReport, Recognizer, Time};

fuzz_target!(|data: &[u8]| {
    let Some(nul) = data.iter().position(|&b| b == 0) else {
        return;
    };
    let (value, rest) = (&data[..nul], &data[nul + 1..]);
    let Some(([lo, hi], steps)) = rest.split_first_chunk() else {
        return;
    };
    let config = Config {
        bindings: Bindings::from_lines(String::from_utf8_lossy(value).split('\n')),
        window: Duration::from_millis(u16::from_le_bytes([*lo, *hi]).into()),
    };
    let mut recognizer = Recognizer::default();
    let mut now = Time::from_elapsed(Duration::ZERO);
    for step in steps.chunks_exact(6) {
        let (after, bytes) = step.split_at(2);
        now = now + Duration::from_millis(u16::from_le_bytes([after[0], after[1]]).into());
        match InputReport::parse(bytes) {
            Some(report) => recognizer.report(now, &report, &config),
            None => recognizer.expire(now, &config),
        }
        // A deadline that has passed would keep the window timer firing.
        if let Some(deadline) = recognizer.deadline() {
            assert!(deadline > now, "deadline {deadline:?} is not after {now:?}");
        }
    }
});
