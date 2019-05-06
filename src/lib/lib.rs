#![allow(unknown_lints)]

#[macro_use]
extern crate err_derive;
extern crate ion_braces as braces;
extern crate ion_lexers as lexers;
extern crate ion_ranges as ranges;
extern crate ion_sys as sys;

#[macro_use]
pub mod types;
#[macro_use]
pub mod parser;
mod ascii_helpers;
pub mod builtins;
mod memory;
pub mod shell;

pub(crate) use self::memory::IonPool;
pub use crate::shell::{
    binary::MAN_ION, flags, pipe_exec::job_control::JobControl, status, Binary, Capture, Fork,
    IonError, IonResult, Shell, ShellBuilder,
};

pub fn version() -> &'static str { include!(concat!(env!("OUT_DIR"), "/version_string")) }

pub fn join_with<S: AsRef<str>>(
    input: &mut small::String,
    with: char,
    mut iter: impl Iterator<Item = S>,
) {
    if let Some(str) = iter.next() {
        input.push_str(str.as_ref());
        iter.for_each(|str| {
            input.push(with);
            input.push_str(str.as_ref());
        });
    }
}

pub fn join<S: AsRef<str>>(input: &mut small::String, mut iter: impl Iterator<Item = S>) {
    if let Some(str) = iter.next() {
        input.push_str(str.as_ref());
        iter.for_each(|str| {
            input.push_str(str.as_ref());
        });
    }
}
