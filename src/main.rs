mod binary;

use self::binary::{InteractiveBinary, MAN_ION};
use ion_shell::{
    builtins::man_pages::check_help, status::Status, types::Str, BuiltinMap, IonError,
    PipelineError, Shell, Value,
};
use ion_sys as sys;
use ion_sys::execve;
use liner::KeyBindings;
use small;
use std::{
    alloc::System,
    env,
    error::Error,
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

const MAN_EXEC: &str = r#"NAME
    exec - Replace the shell with the given command.

SYNOPSIS
    exec [-ch] [--help] [command [arguments ...]]

DESCRIPTION
    Execute <command>, replacing the shell with the specified program.
    The <arguments> following the command become the arguments to
    <command>.

OPTIONS
    -c  Execute command with an empty environment."#;

pub const MAN_EXIT: &str = r#"NAME
    exit - exit the shell

SYNOPSIS
    exit

DESCRIPTION
    Makes ion exit. The exit status will be that of the last command executed."#;

/// Executes the givent commmand.
pub fn exec(args: &[small::String]) -> Result<(), small::String> {
    let mut clear_env = false;
    let mut idx = 0;
    for arg in args.iter() {
        match &**arg {
            "-c" => clear_env = true,
            _ if check_help(args, MAN_EXEC) => {
                return Ok(());
            }
            _ => break,
        }
        idx += 1;
    }

    match args.get(idx) {
        Some(argument) => {
            let args = if args.len() > idx + 1 { &args[idx + 1..] } else { &[] };
            Err(execve(argument, args, clear_env).description().into())
        }
        None => Err("no command provided".into()),
    }
}

fn builtin_exit(args: &[Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_EXIT) {
        return Status::SUCCESS;
    }
    // Kill all active background tasks before exiting the shell.
    shell.background_send(sys::SIGTERM);
    let exit_code = args
        .get(1)
        .and_then(|status| status.parse::<i32>().ok())
        .unwrap_or_else(|| shell.previous_status().as_os_code());
    std::process::exit(exit_code);
}

fn builtin_exec(args: &[Str], _shell: &mut Shell<'_>) -> Status {
    match exec(&args[1..]) {
        // Shouldn't ever hit this case.
        Ok(()) => unreachable!(),
        Err(err) => Status::error(format!("ion: exec: {}", err)),
    }
}

fn main() {
    let mut builtins = BuiltinMap::default().with_shell_unsafe();
    builtins.add("exec", &builtin_exec, "Replace the shell with the given command.");
    builtins.add("exit", &builtin_exit, "Exits the current session");

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
