extern crate getopts;
extern crate ion_shell;
extern crate ion_sys as sys;
extern crate small;
extern crate smallvec;

use getopts::Options;
use ion_shell::{flags::NO_EXEC, Binary, JobControl, ShellBuilder, MAN_ION};
use smallvec::SmallVec;
use std::{
    alloc::System,
    env,
    io::{stdin, BufReader, Read},
};

#[global_allocator]
static A: System = System;

fn main() {
    let stdin_is_a_tty = sys::isatty(sys::STDIN_FILENO);
    let mut shell = ShellBuilder::new().install_signal_handler().block_signals();

    if stdin_is_a_tty {
        shell = shell.set_unique_pid();
    }

    let mut shell = shell.as_binary();

    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optopt(
        "c",
        "command",
        "evaluates given commands instead of reading from the commandline",
        "COMMAND",
    );
    opts.optflag("n", "no-execute", "do not execute any commands, just do syntax checking.");
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("v", "version", "print the version");
    let matches = opts
        .parse(&args[1..])
        .map_err(|e| {
            eprintln!("Error: {}", e);
            std::process::exit(64);
        })
        .unwrap();

    if matches.opt_present("h") {
        println!("{}", opts.usage(MAN_ION));
        return;
    }

    if matches.opt_present("v") {
        println!("{}", ion_shell::version());
        return;
    }

    if matches.opt_present("n") {
        shell.flags |= NO_EXEC;
    }

    let command = matches.opt_str("c");
    let parameters = matches.free.into_iter().map(small::String::from).collect::<SmallVec<_>>();
    let script_path = parameters.get(0).cloned();
    if !parameters.is_empty() {
        shell.variables.set("args", parameters);
    }

    let status = if let Some(command) = command {
        shell.execute_script(&command);
        shell.wait_for_background();
        shell.previous_status
    } else if let Some(path) = script_path {
        shell.execute_file(&path.as_str());
        shell.wait_for_background();
        shell.previous_status
    } else if stdin_is_a_tty {
        shell.execute_interactive();
        unreachable!();
    } else {
        shell.terminate_script_quotes(BufReader::new(stdin()).bytes().filter_map(|b| b.ok()))
    };
    shell.exit(status);
}
