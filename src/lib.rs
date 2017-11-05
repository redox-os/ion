#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]
#![feature(conservative_impl_trait)]
#![feature(integer_atomics)]
#![feature(pointer_methods)]
#![feature(getpid)]

extern crate app_dirs;
#[macro_use]
extern crate bitflags;
extern crate calc;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate errno;
extern crate fnv;
extern crate glob;
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

#[cfg(target_os = "redox")]
#[path = "sys/redox.rs"]
mod sys;

#[cfg(unix)]
#[path = "sys/unix/mod.rs"]
mod sys;

#[macro_use]
mod types;
#[macro_use]
pub mod parser;
mod builtins;
pub mod shell;
mod ascii_helpers;

pub use builtins::Builtin;
pub use shell::Shell;
