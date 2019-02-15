extern crate ion_shell;
extern crate ion_sys as sys;
extern crate smallvec;

use ion_shell::{flags::NO_EXEC, Binary, JobControl, ShellBuilder, MAN_ION};
use smallvec::SmallVec;
use std::{
    alloc::System,
    env,
    error::Error,
    io::{stdin, stdout, BufRead, BufReader, Write},
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

    let mut args = env::args().skip(1);
    while let Some(path) = args.next() {
        match path.as_str() {
            "-n" | "--no-execute" => {
                shell.flags |= NO_EXEC;
                continue;
            }
            "-c" => shell.execute_arguments(args),
            "-v" | "--version" => shell.display_version(),
            "-h" | "--help" => {
                let stdout = stdout();
                let mut stdout = stdout.lock();
                match stdout.write_all(MAN_ION.as_bytes()).and_then(|_| stdout.flush()) {
                    Ok(_) => return,
                    Err(err) => panic!("{}", err.description().to_owned()),
                }
            }
            _ => {
                let mut array = SmallVec::from_iter(Some(path.clone().into()));
                for arg in args {
                    array.push(arg.into());
                }
                shell.variables.set("args", array);
                if let Err(err) = shell.execute_script(&path) {
                    eprintln!("ion: {}", err);
                }
            }
        }

        shell.wait_for_background();
        let previous_status = shell.previous_status;
        shell.exit(previous_status);
    }

    if stdin_is_a_tty {
        shell.execute_interactive();
    } else {
        let reader = BufReader::new(stdin());
        let lines = reader.lines().filter_map(|line| line.ok());
        let status = shell.terminate_script_quotes(lines);
        shell.exit(status);
    }
}
