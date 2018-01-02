extern crate ansi_term;
extern crate version_check;

use ansi_term::Color::{Blue, Red, White};
use version_check::is_nightly;

// Specifies the minimum version needed to compile Ion.
// NOTE: 1.19 is required due to the usage of `break` with values for
// `loop` (RFC 1624, rust-lang/rust GitHub issue #37339).
// const MIN_VERSION: &'static str = "1.19.0";

use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

fn main() {
    match is_nightly() {
        Some(true) => (), // Success!
        Some(false) => {
            eprintln!(
                "{} {}",
                Red.bold().paint("Error:"),
                White.paint("Ion requires nightly to build.")
            );
            // print_version_err(&*version_string);
            eprintln!(
                "{}{}{}",
                Blue.paint("Use `"),
                White.paint("rustup update"),
                Blue.paint("` to update to the latest stable compiler.")
            );
            panic!("Aborting compilation due to incompatible compiler.")
        }
        _ => {
            eprintln!(
                "cargo:warning={}",
                "Ion was unable to check rustc compatibility."
            );
            eprintln!(
                "cargo:warning={}",
                "Build may fail due to incompatible rustc version."
            );
        }
    }
    match write_version_file() {
        Ok(_) => {}
        Err(e) => panic!("Failed to create a version file: {:?}", e),
    }
}

fn write_version_file() -> io::Result<()> {
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let target = env::var("TARGET").unwrap();
    let version_fname = Path::new(&env::var("OUT_DIR").unwrap()).join("version_string");
    let mut version_file = File::create(&version_fname)?;
    write!(
        &mut version_file,
        "r#\"ion {} ({})\nrev {}\"#",
        version,
        target,
        get_git_rev()?.trim()
    )?;
    Ok(())
}

fn get_git_rev() -> io::Result<String> {
    let rev = match Command::new("git").arg("rev-parse").arg("master").output() {
        Ok(out) => match String::from_utf8(out.stdout) {
            Ok(s) => s,
            Err(_) => git_rev_from_file()?,
        },
        Err(_) => git_rev_from_file()?,
    };
    Ok(rev)
}

fn git_rev_from_file() -> io::Result<String> {
    let git_file = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap())
        .join(".git")
        .join("refs")
        .join("heads")
        .join("master");
    let mut file = File::open(git_file)?;
    let mut rev = String::new();
    file.read_to_string(&mut rev)?;
    Ok(rev)
}
