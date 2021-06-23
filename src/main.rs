use self::binary::{builtins, InteractiveShell};
use atty::Stream;
use ion_shell::{BackgroundEvent, BuiltinMap, IonError, PipelineError, Shell, Value};
use liner::KeyBindings;
use nix::{
    sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet, Signal},
    unistd,
};
use std::{
    fs,
    io::{stdin, BufReader},
    process,
    sync::Arc,
};

use crate::binary::MAN_ION;
use std::env;
use thiserror::Error;

mod binary;

struct KeyBindingsWrapper(KeyBindings);

/// The fast, safe, modern rust shell.
/// Ion is a commandline shell created to be a faster and easier to use
/// alternative to the currently available shells. It is not POSIX compliant.
struct CommandLineArgs {
    /// Print the help page of Ion then exit
    help:             bool,
    /// Print the version, platform and revision of Ion then exit
    version:          bool,
    /// Do not execute any commands, perform only syntax checking
    no_execute:       bool,
    /// Use a fake interactive mode, where errors don't exit the shell
    fake_interactive: bool,
    /// Force interactive mode
    interactive:      bool,
    /// Print commands before execution
    print_commands:   bool,
    /// Shortcut layout. Valid options: "vi", "emacs"
    key_bindings:     Option<KeyBindingsWrapper>,
    /// Evaluate given commands instead of reading from the commandline
    command:          Option<String>,
    /// Script arguments (@args). If the -c option is not specified,
    /// the first parameter is taken as a filename to execute
    args:             Vec<String>,
}

#[derive(Debug, Error)]
pub enum ParsingError {
    #[error("flag or option set twice, see --help")]
    ArgTwiceSet,
    #[error("invalid keybinding, see --help")]
    InvalidKeybinding,
}

fn version() -> String { include!(concat!(env!("OUT_DIR"), "/version_string")).to_string() }

fn parse_args() -> Result<CommandLineArgs, ParsingError> {
    let mut arg_twice_set = false;
    let mut invalid_keybinding = false;
    let mut args = env::args().skip(1);
    let mut version = false;
    let mut help = false;
    let mut no_execute = false;
    let mut fake_interactive = false;
    let mut interactive = false;
    let mut print_commands = false;
    let mut key_bindings = None;
    let mut command = None;
    let mut additional_arguments = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-v" | "--version" => {
                if version {
                    arg_twice_set = true;
                }
                version = true;
            }
            "-h" | "--help" => {
                if help {
                    arg_twice_set = true;
                }
                help = true;
            }
            "-n" | "--no-execute" => {
                if no_execute {
                    arg_twice_set = true;
                }
                no_execute = true;
            }
            "-f" | "--fake-interactive" => {
                if fake_interactive {
                    arg_twice_set = true;
                }
                fake_interactive = true;
            }
            "-i" | "--interactive" => {
                if interactive {
                    arg_twice_set = true;
                }
                interactive = true;
            }
            "-x" => {
                if print_commands {
                    arg_twice_set = true;
                }
                print_commands = true;
            }
            "-o" => {
                match key_bindings {
                    Some(KeyBindingsWrapper(KeyBindings::Vi)) => arg_twice_set = true,
                    Some(KeyBindingsWrapper(KeyBindings::Emacs)) => arg_twice_set = true,
                    None => (),
                }
                key_bindings = match args.next().as_deref() {
                    Some("vi") => Some(KeyBindingsWrapper(KeyBindings::Vi)),
                    Some("emacs") => Some(KeyBindingsWrapper(KeyBindings::Emacs)),
                    Some(_) => {
                        invalid_keybinding = true;
                        break;
                    }
                    None => {
                        invalid_keybinding = true;
                        break;
                    }
                }
            }
            "-c" => {
                // convert Option<String< to Option<&str> due to type system limitation
                if let Some(_p) = command.as_deref() {
                    arg_twice_set = true
                }
                command = args.next();
            }
            _ => {
                additional_arguments.push(arg);
            }
        }
    }
    if arg_twice_set {
        return Err(ParsingError::ArgTwiceSet);
    }
    if invalid_keybinding {
        return Err(ParsingError::InvalidKeybinding);
    }
    // bubble up errors
    Ok(CommandLineArgs {
        help,
        version,
        no_execute,
        fake_interactive,
        interactive,
        print_commands,
        key_bindings,
        command,
        args: additional_arguments,
    })
}

fn set_unique_pid() -> nix::Result<()> {
    let pgid = unistd::getpid();
    if pgid != unistd::getpgrp() {
        unistd::setpgid(pgid, pgid)?;
    }
    if pgid != unistd::tcgetpgrp(nix::libc::STDIN_FILENO)? {
        unsafe { signal::signal(Signal::SIGTTOU, SigHandler::SigIgn) }?;
        unistd::tcsetpgrp(nix::libc::STDIN_FILENO, pgid)?;
    }
    Ok(())
}

fn main() {
    let parsedargs = parse_args();
    let command_line_args = match parsedargs {
        Ok(parsedargs) => parsedargs,
        Err(ParsingError::ArgTwiceSet) => {
            eprintln!("flag or option set twice, see --help");
            process::exit(1);
        }
        Err(ParsingError::InvalidKeybinding) => {
            eprintln!("invalid keybinding, see --help");
            process::exit(1);
        }
    };

    if command_line_args.help {
        println!("{}", MAN_ION);
        return;
    }
    if command_line_args.version {
        println!("{}", version());
        return;
    }
    if command_line_args.command.is_some() && !command_line_args.args.is_empty() {
        eprintln!("either execute command or file(s)");
        process::exit(1);
    }

    let mut builtins = BuiltinMap::default();
    builtins
        .with_unsafe()
        .add("debug", &builtins::builtin_debug, "Toggle debug mode (print commands on exec)")
        .add("exec", &builtins::builtin_exec, "Replace the shell with the given command.")
        .add("exit", &builtins::builtin_exit, "Exits the current session")
        .add("suspend", &builtins::builtin_suspend, "Suspends the shell with a SIGTSTOP signal");

    let stdin_is_a_tty = atty::is(Stream::Stdin);
    let mut shell = Shell::with_builtins(builtins);

    if stdin_is_a_tty {
        if let Err(err) = set_unique_pid() {
            println!("ion: could not bring shell to foreground: {}", err);
        }
    }

    shell.set_background_event(Some(Arc::new(|njob, pid, kind| match kind {
        BackgroundEvent::Added => eprintln!("ion: bg [{}] {}", njob, pid),
        BackgroundEvent::Stopped => eprintln!("ion: ([{}] {}) Stopped", njob, pid),
        BackgroundEvent::Resumed => eprintln!("ion: ([{}] {}) Running", njob, pid),
        BackgroundEvent::Exited(status) => {
            eprintln!("ion: ([{}] {}) exited with {}", njob, pid, status)
        }
        BackgroundEvent::Errored(error) => {
            eprintln!("ion: ([{}] {}) errored: {}", njob, pid, error)
        }
    })));

    shell.opts_mut().no_exec = command_line_args.no_execute;
    shell.opts_mut().grab_tty = stdin_is_a_tty;
    if command_line_args.print_commands {
        shell.set_pre_command(Some(Box::new(|_shell, pipeline| {
            // A string representing the command is stored here.
            eprintln!("> {}", pipeline);
        })));
    }

    let script_path = command_line_args.args.get(0).cloned();
    shell.variables_mut().set(
        "args",
        Value::Array(
            if script_path.is_some() {
                command_line_args.args
            } else {
                vec![std::env::args().next().unwrap()]
            }
            .into_iter()
            .map(|arg| Value::Str(arg.into()))
            .collect(),
        ),
    );

    let err = if let Some(command) = command_line_args.command {
        shell.execute_command(command.as_bytes())
    } else if let Some(path) = script_path {
        match fs::File::open(&path) {
            Ok(script) => shell.execute_command(std::io::BufReader::new(script)),
            Err(cause) => {
                println!("ion: could not execute '{}': {}", path, cause);
                process::exit(1);
            }
        }
    } else if stdin_is_a_tty || command_line_args.interactive {
        let mut interactive = InteractiveShell::new(shell);
        if let Some(key_bindings) = command_line_args.key_bindings {
            interactive.set_keybindings(key_bindings.0);
        }
        interactive.add_callbacks();
        interactive.execute_interactive();
    } else if command_line_args.fake_interactive {
        let mut reader = BufReader::new(stdin());
        loop {
            if let Err(err) = shell.execute_command(&mut reader) {
                eprintln!("ion: {}", err);
            }
        }
    } else {
        shell.execute_command(BufReader::new(stdin()))
    }
    .and_then(|_| shell.wait_for_background().map_err(Into::into));
    if let Err(IonError::PipelineExecutionError(PipelineError::Interrupted(_, signal))) = err {
        // When the job was aborted because of an interrupt signal, abort with this same signal
        let action = SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty());
        let _ = unsafe { nix::sys::signal::sigaction(signal, &action) };
        let _ = nix::sys::signal::raise(signal);
    }
    if let Err(why) = err {
        eprintln!("ion: {}", why);
        process::exit(1);
    }
    process::exit(shell.previous_status().as_os_code());
}
