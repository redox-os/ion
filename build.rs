extern crate ansi_term;
extern crate peg;
extern crate version_check;

use ansi_term::Color::{Red, Yellow, Blue, White};
use version_check::is_min_version;

// Specifies the minimum version needed to compile Ion.
// NOTE: 1.19 is required due to the usage of `break` with values for
// `loop` (RFC 1624, rust-lang/rust GitHub issue #37339).
const MIN_VERSION: &'static str = "1.19.0";

// Convenience macro for writing to stderr.
macro_rules! printerr {
    ($($arg:tt)*) => ({
        use std::io::prelude::*;
        write!(&mut ::std::io::stderr(), "{}\n", format_args!($($arg)*))
            .expect("Failed to write to stderr.")
    })
}

fn main() {
    let print_version_err = |version: &str| {
        printerr!("{} {}. {} {}.",
                  White.paint("Installed version is:"),
                  Yellow.paint(format!("{}", version)),
                  White.paint("Minimum required:"),
                  Yellow.paint(format!("{}", MIN_VERSION)));
    };

    match is_min_version(MIN_VERSION) {
        Some((is_minimum, _)) if is_minimum => (), // Success!
        Some((_, ref version_string)) => {
            printerr!("{} {}",
                      Red.bold().paint("Error:"),
                      White.paint("Ion requires at least version 1.19.0 to build."));
            print_version_err(&*version_string);
            printerr!("{}{}{}",
                Blue.paint("Use `"),
                White.paint("rustup update"),
                Blue.paint("` to update to the latest stable compiler."));
            panic!("Aborting compilation due to incompatible compiler.")
        },
        _ => {
            println!("cargo:warning={}", "Ion was unable to check rustc compatibility.");
            println!("cargo:warning={}", "Build may fail due to incompatible rustc version.");
        }
    }

    peg::cargo_build("src/parser/grammar.rustpeg");
}
