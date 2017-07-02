#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]

// For a performance boost on Linux
// #![feature(alloc_system)]
// extern crate alloc_system;

#[macro_use]
extern crate bitflags;
extern crate fnv;
extern crate glob;
extern crate liner;
extern crate smallvec;
extern crate smallstring;

#[cfg(not(target_os = "redox"))] extern crate futures;
#[cfg(not(target_os = "redox"))] extern crate libc;
#[cfg(not(target_os = "redox"))] extern crate nix;
#[cfg(not(target_os = "redox"))] extern crate tokio_core;
#[cfg(not(target_os = "redox"))] extern crate tokio_signal;

#[cfg(all(unix, not(target_os = "redox")))]
extern crate users as users_unix;

#[macro_use] mod parser;
mod builtins;
mod shell;
mod ascii_helpers;
mod types;

use std::io::{stderr, Write, ErrorKind};

use builtins::Builtin;
use shell::Shell;

#[cfg(not(target_os = "redox"))] use tokio_core::reactor::Core;
#[cfg(not(target_os = "redox"))] use futures::{Future, Stream};
#[cfg(not(target_os = "redox"))] use tokio_signal::unix::{self as unix_signal, Signal};

use std::sync::mpsc;
use std::thread;

fn inner_main(sigint_rx : mpsc::Receiver<i32>) {
   let builtins = Builtin::map();
   let mut shell = Shell::new(&builtins, sigint_rx);
   shell.evaluate_init_file();

   if "1" == shell.variables.get_var_or_empty("HISTORY_FILE_ENABLED") {
       shell.context.history.set_file_name(shell.variables.get_var("HISTORY_FILE"));
       match shell.context.history.load_history() {
           Ok(()) => {
               // pass
           }
           Err(ref err) if err.kind() == ErrorKind::NotFound => {
               let history_filename = shell.variables.get_var_or_empty("HISTORY_FILE");
               let _ = writeln!(stderr(), "ion: failed to find history file {}: {}", history_filename, err);
           },
           Err(err) => {
               let _ = writeln!(stderr(), "ion: failed to load history: {}", err);
           }
       }
   }
   shell.execute();
}


#[cfg(not(target_os = "redox"))]
fn main() {
    let (signals_tx, signals_rx) = mpsc::channel();

    thread::spawn(move || inner_main(signals_rx));

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // Create a stream that will select over SIGINT, SIGTERM and SIGTSTP signals.
    let signal_stream = Signal::new(unix_signal::SIGINT, &handle).flatten_stream()
        .select(Signal::new(unix_signal::SIGTERM, &handle).flatten_stream())
        .select(Signal::new(libc::SIGTSTP, &handle).flatten_stream());

    // Execute the event loop that will listen for and transmit received
    // signals to the shell.
    core.run(signal_stream.for_each(|signal| {
        let _ = signals_tx.send(signal);
        Ok(())
    })).unwrap();
}

#[cfg(target_os = "redox")]
fn main() {
    let (_, signals_rx) = mpsc::channel();
    inner_main(signals_rx);
}
