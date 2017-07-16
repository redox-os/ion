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
#[cfg(all(unix, not(target_os = "redox")))] extern crate futures;
#[cfg(all(unix, not(target_os = "redox")))] extern crate libc;
#[cfg(all(unix, not(target_os = "redox")))] extern crate nix;
#[cfg(all(unix, not(target_os = "redox")))] extern crate tokio_core;
#[cfg(all(unix, not(target_os = "redox")))] extern crate tokio_signal;
#[cfg(all(unix, not(target_os = "redox")))] extern crate users as users_unix;
#[cfg(target_os = "redox")] extern crate syscall;

#[macro_use] mod types;
#[macro_use] mod parser;
mod builtins;
mod shell;
mod ascii_helpers;

use builtins::Builtin;
use shell::{Shell, signals};
use std::sync::mpsc;
use std::thread;

fn inner_main(sigint_rx : mpsc::Receiver<i32>) {
    let builtins = Builtin::map();
    let mut shell = Shell::new(&builtins, sigint_rx);
    shell.evaluate_init_file();
    shell.execute();
}

#[cfg(not(target_os = "redox"))]
fn main() {
    let (signals_tx, signals_rx) = mpsc::channel();
    thread::spawn(move || inner_main(signals_rx));
    signals::event_loop(signals_tx);
}

#[cfg(target_os = "redox")]
fn main() {
    let (_, signals_rx) = mpsc::channel();
    inner_main(signals_rx);
}
