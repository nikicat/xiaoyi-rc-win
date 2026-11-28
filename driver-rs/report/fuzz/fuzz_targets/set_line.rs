//! `Bindings::set_line`, on any line. A rejected line changes nothing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use xiaoyi_rc_report::Bindings;

fuzz_target!(|line: &str| {
    let mut bindings = Bindings::from_lines(["E9 E9 = E2", "EA = 0"]);
    let before = bindings.clone();
    if bindings.set_line(line).is_err() {
        assert_eq!(bindings, before);
    }
});
