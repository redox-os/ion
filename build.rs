extern crate ansi_term;
extern crate version_check;

use ansi_term::Color::{Blue, Red, White, Yellow};
use version_check::is_min_version;

// Specifies the minimum version needed to compile Ion.
// NOTE: 1.19 is required due to the usage of `break` with values for
// `loop` (RFC 1624, rust-lang/rust GitHub issue #37339).
const MIN_VERSION: &'static str = "1.19.0";

use std::env;
use std::path::Path;
use std::fs::File;
use std::io::{self, Write, Read};

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
        printerr!(
            "{} {}. {} {}.",
            White.paint("Installed version is:"),
            Yellow.paint(format!("{}", version)),
            White.paint("Minimum required:"),
            Yellow.paint(format!("{}", MIN_VERSION))
        );
    };

    match is_min_version(MIN_VERSION) {
        Some((is_minimum, _)) if is_minimum => (), // Success!
        Some((_, ref version_string)) => {
            printerr!(
                "{} {}",
                Red.bold().paint("Error:"),
                White.paint("Ion requires at least version 1.19.0 to build.")
            );
            print_version_err(&*version_string);
            printerr!(
                "{}{}{}",
                Blue.paint("Use `"),
                White.paint("rustup update"),
                Blue.paint("` to update to the latest stable compiler.")
            );
            panic!("Aborting compilation due to incompatible compiler.")
        }
        _ => {
            println!("cargo:warning={}", "Ion was unable to check rustc compatibility.");
            println!("cargo:warning={}", "Build may fail due to incompatible rustc version.");
        }
    }
    match write_version_file() {
        Ok(_) => {},
        Err(e) => panic!("Failed to create a version file: {:?}", e),
    }
}

fn write_version_file() -> io::Result<()> {
    // get the .git/refs/head/master file and read that for the rev
    let git_file = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join(".git")
        .join("refs")
        .join("heads")
        .join("master");
    println!("opening {:?}", git_file);
    let mut file = File::open(git_file)?;
    let mut rev = String::new();
    file.read_to_string(&mut rev)?;
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let target = env::var("TARGET").unwrap();
    let version_fname = Path::new(&env::var("OUT_DIR").unwrap()).join("version_string");
    let mut version_file = File::create(&version_fname)?;
    write!(&mut version_file, "r#\"ion {} ({})\nrev {}\"#", version, target, rev.trim())?;
    Ok(())
}