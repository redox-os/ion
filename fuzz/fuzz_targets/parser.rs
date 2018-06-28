#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate ion_shell;

use ion_shell::parser::fuzzing::*;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = statement_parse(&s);
    }
});
