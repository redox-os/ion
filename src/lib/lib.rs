#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]
#![feature(integer_atomics)]
#![feature(pointer_methods)]
#![feature(getpid)]
#![feature(nll)]

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
extern crate libc;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate libloading;
extern crate liner;
extern crate regex;
#[macro_use]
extern crate serde_derive;
extern crate smallstring;
extern crate smallvec;
#[cfg(target_os = "redox")]
extern crate syscall;
extern crate toml;
extern crate unicode_segmentation;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate users as users_unix;
extern crate xdg;

#[cfg(target_os = "redox")]
#[path = "sys/redox/mod.rs"]
pub mod sys;

#[cfg(unix)]
#[path = "sys/unix/mod.rs"]
pub mod sys;

#[macro_use]
mod types;
#[macro_use]
pub mod parser;
mod ascii_helpers;
mod builtins;
mod shell;

pub use shell::{
    binary::MAN_ION, flags, pipe_exec::job_control::JobControl, status, Binary, Capture, Fork,
    IonError, IonResult, Shell, ShellBuilder,
};
