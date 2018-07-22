#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]

#[macro_use]
extern crate bitflags;
extern crate calc;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate fnv;
extern crate glob;
extern crate itoa;
#[macro_use]
extern crate lazy_static;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate libloading;
extern crate liner;
extern crate regex;
extern crate small;
extern crate smallvec;
extern crate unicode_segmentation;
extern crate xdg;

extern crate ion_sys as sys;
extern crate ion_braces as braces;
extern crate ion_builtins;
extern crate ion_lexers as lexers;
extern crate ion_ranges as ranges;

#[macro_use]
pub mod types;
#[macro_use]
pub mod parser;
mod ascii_helpers;
mod builtins;
pub mod shell;

pub use shell::{
    binary::MAN_ION, flags, pipe_exec::job_control::JobControl, status, Binary, Capture, Fork,
    IonError, IonResult, Shell, ShellBuilder,
};
