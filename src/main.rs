extern crate ion_sys as sys;

use ion_shell::{flags::NO_EXEC, Binary, JobControl, ShellBuilder, MAN_ION};
use smallvec::SmallVec;
use std::{
    alloc::System,
    env,
    io::{stdin, BufReader},
    iter::FromIterator,
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

    let mut command = None;
    let mut args = env::args().skip(1);
    let mut script_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-n" | "--no-execute" => {
                shell.flags |= NO_EXEC;
            }
            "-c" => command = args.next(),
            "-v" | "--version" => {
                println!("{}", ion_shell::version());
                return;
            }
            "-h" | "--help" => {
                println!("{}", MAN_ION);
                return;
            }
            _ => {
                script_path = Some(arg);
                break;
            }
        }
    }

    shell.variables.set(
        "args",
        SmallVec::from_iter(
            script_path
                .clone()
                .or(env::args().next())
                .into_iter()
                .chain(args)
                .map(|arg| arg.into()),
        ),
    );

    if let Some(command) = command {
        shell.execute_script(command.as_bytes());
    } else if let Some(path) = script_path {
        shell.execute_file(&path.as_str());
    } else if stdin_is_a_tty {
        shell.execute_interactive();
        unreachable!();
    } else {
        shell.execute_script(BufReader::new(stdin()));
    }
    shell.wait_for_background();
    shell.exit(shell.previous_status);
}
