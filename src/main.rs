extern crate ion_sys as sys;

use ion_shell::{InteractiveBinary, Shell, Value, MAN_ION};
use liner::KeyBindings;
use std::{
    alloc::System,
    env,
    io::{stdin, BufReader},
    process,
};

#[global_allocator]
static A: System = System;

fn main() {
    let stdin_is_a_tty = sys::isatty(sys::STDIN_FILENO);
    let mut shell = Shell::binary();

    if stdin_is_a_tty {
        shell.set_unique_pid();
    }

    let mut command = None;
    let mut args = env::args().skip(1);
    let mut script_path = None;
    let mut key_bindings = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" => match args.next().as_ref().map(|s| s.as_str()) {
                Some("vi") => key_bindings = Some(KeyBindings::Vi),
                Some("emacs") => key_bindings = Some(KeyBindings::Emacs),
                Some(_) => {
                    eprintln!("ion: invalid option for option -o");
                    process::exit(1);
                }
                None => {
                    eprintln!("ion: no option given for option -o");
                    process::exit(1);
                }
            },
            "-x" => shell.opts_mut().print_comms = true,
            "-n" | "--no-execute" => shell.opts_mut().no_exec = true,
            "-c" => command = args.next(),
            "-v" | "--version" => {
                println!(include!(concat!(env!("OUT_DIR"), "/version_string")));
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

    shell.variables_mut().set(
        "args",
        Value::Array(
            script_path
                .clone()
                .or_else(|| env::args().next())
                .into_iter()
                .chain(args)
                .map(|arg| Value::Str(arg.into()))
                .collect(),
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
        interactive.add_callbacks();
        interactive.execute_interactive();
    } else {
        shell.execute_script(BufReader::new(stdin()));
    }
    shell.wait_for_background();
    shell.exit(None);
}
