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
#[macro_use] extern crate lazy_static;
extern crate liner;
extern crate smallvec;
extern crate smallstring;
extern crate calc;
extern crate regex;
#[cfg(all(unix, not(target_os = "redox")))] extern crate libc;
#[cfg(all(unix, not(target_os = "redox")))] extern crate nix;
#[cfg(all(unix, not(target_os = "redox")))] extern crate users as users_unix;
#[cfg(target_os = "redox")] extern crate syscall;

#[cfg(target_os = "redox")]
#[path="sys/redox.rs"]
mod sys;

#[cfg(unix)]
#[path="sys/unix.rs"]
mod sys;

#[macro_use] mod types;
#[macro_use] mod parser;
mod builtins;
mod shell;
mod ascii_helpers;

use builtins::Builtin;
use shell::{Shell, Binary};
use std::sync::mpsc;
use std::{thread, time};

static mut SIGNALS_TX: *const mpsc::Sender<i32> = 0 as *const mpsc::Sender<i32>;

extern "C" fn handler(signal: i32) {
    let signals_tx = unsafe { SIGNALS_TX };
    if signals_tx as usize != 0 {
        let _ = unsafe { (*signals_tx).send(signal) };
    }
}

fn inner_main(sigint_rx : mpsc::Receiver<i32>) {
    let builtins = Builtin::map();
    let shell = Shell::new(&builtins, sigint_rx);
    shell.main();
}

fn main() {
    let (signals_tx, signals_rx) = mpsc::channel();
    unsafe {
        SIGNALS_TX = Box::into_raw(Box::new(signals_tx));
    }

    let _ = sys::signal(sys::SIGHUP, handler);
    let _ = sys::signal(sys::SIGINT, handler);
    let _ = sys::signal(sys::SIGTERM, handler);

    if let Ok(pid) = sys::getpid() {
        if sys::setpgid(0, pid).is_ok() {
            let _ = sys::tcsetpgrp(0, pid);
        }
    }

    thread::spawn(move || inner_main(signals_rx));
    loop {
        thread::sleep(time::Duration::new(1, 0));
    }
}
