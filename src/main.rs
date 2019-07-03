use self::binary::{builtins, InteractiveShell};
use atty::Stream;
use ion_shell::{BuiltinMap, Shell, Value};
use liner::KeyBindings;
use nix::{
    sys::signal::{self, SigHandler, Signal},
    unistd,
};
use std::{
    fs,
    io::{stdin, BufReader},
    process,
};

#[cfg(not(feature = "advanced_arg_parsing"))]
use crate::binary::MAN_ION;
#[cfg(not(feature = "advanced_arg_parsing"))]
use std::env;
#[cfg(feature = "advanced_arg_parsing")]
use std::str::FromStr;
#[cfg(feature = "advanced_arg_parsing")]
use structopt::StructOpt;

mod binary;

struct KeyBindingsWrapper(KeyBindings);

#[cfg(feature = "advanced_arg_parsing")]
impl FromStr for KeyBindingsWrapper {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "vi" => Ok(KeyBindingsWrapper(KeyBindings::Vi)),
            "emacs" => Ok(KeyBindingsWrapper(KeyBindings::Emacs)),
            _ => Err("unknown key bindings".to_string()),
        }
    }
}

/// Ion is a commandline shell created to be a faster and easier to use alternative to the
/// currently available shells. It is not POSIX compliant.
#[cfg_attr(feature = "advanced_arg_parsing", derive(StructOpt))]
#[cfg_attr(
    feature = "advanced_arg_parsing",
    structopt(
        name = "Ion - The Ion Shell",
        author = "",
        raw(setting = "structopt::clap::AppSettings::ColoredHelp")
    )
)]
struct CommandLineArgs {
    /// Shortcut layout. Valid options: "vi", "emacs"
    #[cfg_attr(feature = "advanced_arg_parsing", structopt(short = "-o"))]
    key_bindings: Option<KeyBindingsWrapper>,
    /// Print commands before execution
    #[cfg_attr(feature = "advanced_arg_parsing", structopt(short = "-x"))]
    print_commands: bool,
    /// Force interactive mode
    #[cfg_attr(feature = "advanced_arg_parsing", structopt(short = "-i", long = "--interactive"))]
    interactive: bool,
    /// Do not execute any commands, perform only syntax checking
    #[cfg_attr(feature = "advanced_arg_parsing", structopt(short = "-n", long = "--no-execute"))]
    no_execute: bool,
    /// Evaluate given commands instead of reading from the commandline
    #[cfg_attr(feature = "advanced_arg_parsing", structopt(short = "-c"))]
    command: Option<String>,
    /// Print the version, platform and revision of Ion then exit
    #[cfg_attr(feature = "advanced_arg_parsing", structopt(short = "-v", long = "--version"))]
    version: bool,
    /// Script arguments (@args). If the -c option is not specified,
    /// the first parameter is taken as a filename to execute
    #[cfg_attr(feature = "advanced_arg_parsing", structopt())]
    args: Vec<String>,
}

fn version() -> String { include!(concat!(env!("OUT_DIR"), "/version_string")).to_string() }

#[cfg(feature = "advanced_arg_parsing")]
fn parse_args() -> CommandLineArgs { CommandLineArgs::from_args() }

#[cfg(not(feature = "advanced_arg_parsing"))]
fn parse_args() -> CommandLineArgs {
    let mut args = env::args().skip(1);
    let mut command = None;
    let mut key_bindings = None;
    let mut no_execute = false;
    let mut print_commands = false;
    let mut interactive = false;
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
            _ => {
                additional_arguments.push(arg);
            }
        }
    }
    CommandLineArgs {
        key_bindings,
        print_commands,
        interactive,
        no_execute,
        command,
        version,
        args: additional_arguments,
    }
}

fn set_unique_pid() -> nix::Result<()> {
    let pgid = unistd::getpid();
    if pgid == unistd::tcgetpgrp(nix::libc::STDIN_FILENO)? {
        return Ok(());
    }
    unistd::setpgid(pgid, pgid)?;
    unsafe { signal::signal(Signal::SIGTTOU, SigHandler::SigIgn) }?;
    unistd::tcsetpgrp(nix::libc::STDIN_FILENO, pgid)
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
            command_line_args.args.into_iter().map(|arg| Value::Str(arg.into())).collect(),
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
    } else {
        shell.execute_command(BufReader::new(stdin()))
    };
    if let Err(why) = err {
        eprintln!("ion: {}", why);
        process::exit(1);
    }
    if let Err(why) = shell.wait_for_background() {
        eprintln!("ion: {}", why);
        process::exit(1);
    }
    process::exit(shell.previous_status().as_os_code());
}
