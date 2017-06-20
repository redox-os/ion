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

use std::sync::mpsc;
use std::thread;

fn inner_main(sigint_rx : mpsc::Receiver<bool>) {
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
    let (sigint_tx, sigint_rx) = mpsc::channel();

    thread::spawn(move || inner_main(sigint_rx));

    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let ctrl_c = tokio_signal::ctrl_c(&handle).flatten_stream();
    let signal_handler = ctrl_c.for_each(|()| {
        eprintln!("ion: received SIGINT (Ctrl+C)");
        let _ = sigint_tx.send(true);
        Ok(())
    });
    core.run(signal_handler).unwrap();
}

#[cfg(target_os = "redox")]
fn main() {
    let (_, sigint_rx) = mpsc::channel();
    inner_main(sigint_rx);
}
