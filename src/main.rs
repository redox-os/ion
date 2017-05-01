#![allow(unknown_lints)]
#![allow(while_let_on_iterator)]

// For a performance boost on Linux
// #![feature(alloc_system)]
// extern crate alloc_system;

extern crate fnv;
extern crate glob;
extern crate liner;
extern crate smallvec;
extern crate smallstring;

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

fn main() {
    let builtins = Builtin::map();
    let mut shell = Shell::new(&builtins);
    shell.evaluate_init_file();

    if "1" == shell.variables.get_var_or_empty("HISTORY_FILE_ENABLED") {
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
