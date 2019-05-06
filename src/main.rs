extern crate ion_sys as sys;

use ion_shell::{
    flags::{NO_EXEC, PRINT_COMMS},
    InteractiveBinary, JobControl, ShellBuilder, MAN_ION,
};
use liner::KeyBindings;
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
    let mut key_bindings = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" => match args.next().as_ref().map(|s| s.as_str()) {
                Some("vi") => {
                    key_bindings = Some(KeyBindings::Vi);
                }
                Some("emacs") => {
                    key_bindings = Some(KeyBindings::Emacs);
                }
                Some(_) => {
                    eprintln!("ion: set: invalid option");
                    return;
                }
                None => {
                    eprintln!("ion: set: no option given");
                    return;
                }
            },
            "-x" => shell.flags |= PRINT_COMMS,
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
                .or_else(|| env::args().next())
                .into_iter()
                .chain(args)
                .map(Into::into),
        ),
    );

    if let Some(command) = command {
        shell.execute_script(command.as_bytes());
    } else if let Some(path) = script_path {
        shell.execute_file(&path.as_str());
    } else if stdin_is_a_tty {
        let mut interactive = InteractiveBinary::new(shell);
        if let Some(key_bindings) = key_bindings {
            interactive.set_keybindings(key_bindings);
        }
        interactive.init_file();
        interactive.add_callbacks();
        interactive.execute_interactive();
    } else {
        shell.execute_script(BufReader::new(stdin()));
    }
    shell.wait_for_background();
    shell.exit(shell.previous_status);
}
