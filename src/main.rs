mod binary;

use self::binary::{builtins, InteractiveBinary, MAN_ION};
use ion_shell::{BuiltinMap, IonError, PipelineError, Shell, Value};
use ion_sys as sys;
use liner::KeyBindings;
use std::{
    alloc::System,
    env,
    io::{self, stdin, BufReader},
    process,
};

#[global_allocator]
static A: System = System;

fn set_unique_pid() -> io::Result<()> {
    let pid = sys::getpid()?;
    sys::setpgid(0, pid)?;
    sys::tcsetpgrp(0, pid)
}

fn main() {
    let mut builtins = BuiltinMap::default().with_shell_unsafe();
    builtins.add("exec", &builtins::exec, "Replace the shell with the given command.");
    builtins.add("exit", &builtins::exit, "Exits the current session");

    let stdin_is_a_tty = sys::isatty(sys::STDIN_FILENO);
    let mut shell = Shell::with_builtins(builtins, false);

    if stdin_is_a_tty {
        if let Err(why) = set_unique_pid() {
            eprintln!("ion: could not assign a pid to the shell: {}", why);
        }
    }

    let mut command = None;
    let mut args = env::args().skip(1);
    let mut script_path = None;
    let mut key_bindings = None;
    let mut force_interactive = false;
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
            "-i" | "--interactive" => force_interactive = true,
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

    let err = if let Some(command) = command {
        shell.execute_command(command.as_bytes())
    } else if let Some(path) = script_path {
        shell.execute_file(&path.as_str())
    } else if stdin_is_a_tty || force_interactive {
        let mut interactive = InteractiveBinary::new(shell);
        if let Some(key_bindings) = key_bindings {
            interactive.set_keybindings(key_bindings);
        }
        interactive.add_callbacks();
        interactive.execute_interactive();
    } else {
        shell.execute_command(BufReader::new(stdin()))
    };
    if let Err(why) = err {
        eprintln!("ion: {}", why);
        process::exit(
            if let IonError::PipelineExecutionError(PipelineError::Interrupted(_, signal)) = why {
                signal
            } else {
                1
            },
        );
    }
    if let Err(why) = shell.wait_for_background() {
        eprintln!("ion: {}", why);
        process::exit(if let PipelineError::Interrupted(_, signal) = why { signal } else { 1 });
    }
    process::exit(shell.previous_status().as_os_code());
}
