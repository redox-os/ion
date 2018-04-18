#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]
#![feature(conservative_impl_trait)]
#![feature(integer_atomics)]
#![feature(pointer_methods)]
#![feature(getpid)]

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
extern crate smallstring;
extern crate smallvec;
#[cfg(target_os = "redox")]
extern crate syscall;
extern crate unicode_segmentation;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate users as users_unix;
extern crate xdg;

#[cfg(target_os = "redox")]
#[path = "sys/redox/mod.rs"]
mod sys;

#[cfg(unix)]
#[path = "sys/unix/mod.rs"]
mod sys;

#[macro_use]
mod types;
#[macro_use]
pub mod parser;
mod builtins;
mod shell;
mod ascii_helpers;

pub use shell::{Binary, Capture, Fork, IonError, IonResult, Shell, ShellBuilder};
pub use shell::binary::MAN_ION;
pub use shell::flags;
pub use shell::pipe_exec::job_control::JobControl;
pub use shell::status;
