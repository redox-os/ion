#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]

// For a performance boost on Linux
// #![feature(alloc_system)]
// extern crate alloc_system;

extern crate app_dirs;
#[macro_use]
extern crate bitflags;
extern crate fnv;
extern crate glob;
#[macro_use]
extern crate lazy_static;
extern crate liner;
extern crate smallvec;
extern crate smallstring;
extern crate calc;
extern crate regex;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate libc;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate nix;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate users as users_unix;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate libloading;
#[cfg(target_os = "redox")]
extern crate syscall;

#[cfg(target_os = "redox")]
#[path = "sys/redox.rs"]
mod sys;

#[cfg(unix)]
#[path = "sys/unix.rs"]
mod sys;

#[macro_use]
mod types;
#[macro_use]
mod parser;
mod builtins;
mod shell;
mod ascii_helpers;

use builtins::Builtin;
use shell::{Binary, Shell, signals};
use std::sync::atomic::Ordering;

extern "C" fn handler(signal: i32) {
    if signal < 32 {
        signals::PENDING.fetch_or(1 << signal, Ordering::SeqCst);
    }
}

fn main() {
    let _ = sys::signal(sys::SIGHUP, handler);
    let _ = sys::signal(sys::SIGINT, handler);
    let _ = sys::signal(sys::SIGTERM, handler);

    // This will block SIGTSTP, SIGTTOU, SIGTTIN, and SIGCHLD, which is required
    // for this shell to manage its own process group / children / etc.
    signals::block();

    if let Ok(pid) = sys::getpid() {
        if sys::setpgid(0, pid).is_ok() {
            let _ = sys::tcsetpgrp(0, pid);
        }
    }

    let builtins = Builtin::map();
    let shell = Shell::new(&builtins);
    shell.main();
}
