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

mod binary;

struct KeyBindingsWrapper(KeyBindings);


/// The fast, safe, modern rust shell.
/// Ion is a commandline shell created to be a faster and easier to use
/// alternative to the currently available shells. It is not POSIX compliant.
struct CommandLineArgs {
    /// Shortcut layout. Valid options: "vi", "emacs"
    key_bindings:     Option<KeyBindingsWrapper>,
    /// Print commands before execution
    print_commands:   bool,
    /// Use a fake interactive mode, where errors don't exit the shell
    fake_interactive: bool,
    /// Force interactive mode
    interactive:      bool,
    /// Do not execute any commands, perform only syntax checking
    no_execute:       bool,
    /// Evaluate given commands instead of reading from the commandline
    command:          Option<String>,
    /// Print the version, platform and revision of Ion then exit
    version:          bool,
    /// Script arguments (@args). If the -c option is not specified,
    /// the first parameter is taken as a filename to execute
    args:             Vec<String>,
}

fn version() -> String { include!(concat!(env!("OUT_DIR"), "/version_string")).to_string() }

fn parse_args() -> CommandLineArgs {
    let mut args = env::args().skip(1);
    let mut command = None;
    let mut key_bindings = None;
    let mut no_execute = false;
    let mut print_commands = false;
    let mut interactive = false;
    let mut fake_interactive = false;
    let mut version = false;
    let mut additional_arguments = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" => {
                key_bindings = match args.next().as_ref().map(|s| s.as_str()) {
                    Some("vi") => Some(KeyBindingsWrapper(KeyBindings::Vi)),
                    Some("emacs") => Some(KeyBindingsWrapper(KeyBindings::Emacs)),
                    Some(_) => {
                        eprintln!("ion: invalid option for option -o");
                        process::exit(1);
                    }
                    None => {
                        eprintln!("ion: no option given for option -o");
                        process::exit(1);
                    }
                }
            }
            "-x" => print_commands = true,
            "-n" | "--no-execute" => no_execute = true,
            "-c" => command = args.next(),
            "-v" | "--version" => version = true,
            "-h" | "--help" => {
                println!("{}", MAN_ION);
                process::exit(0);
            }
            "-i" | "--interactive" => interactive = true,
            "-f" | "--fake-interactive" => fake_interactive = true,
            _ => {
                additional_arguments.push(arg);
            }
        }
    }
    CommandLineArgs {
        key_bindings,
        print_commands,
        interactive,
        fake_interactive,
        no_execute,
        command,
        version,
        args: additional_arguments,
    }
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
    let command_line_args = parse_args();

    if command_line_args.version {
        println!("{}", version());
        return;
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
