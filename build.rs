// Specifies the minimum version needed to compile Ion.
// NOTE: 1.19 is required due to the usage of `break` with values for
// `loop` (RFC 1624, rust-lang/rust GitHub issue #37339).
// const MIN_VERSION: &'static str = "1.19.0";

use std::{
    env,
    fs::{self, File},
    io::{self, Read, Write},
    path::Path,
    process::Command,
};

fn main() {
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
    let version_file = Path::new("git_revision.txt");
    if version_file.exists() {
        fs::read_to_string(&version_file)
    } else {
        Command::new("git")
            .arg("rev-parse")
            .arg("master")
            .output()
            .and_then(|out| {
                String::from_utf8(out.stdout).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "git rev-parse master output was not UTF-8",
                    )
                })
            })
            .or_else(|_| git_rev_from_file())
    }
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
