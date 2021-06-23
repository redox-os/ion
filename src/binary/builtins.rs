use ion_shell::{builtin, builtins::Status, types::Str, Shell, Signal};
use nix::{sys::signal, unistd::Pid};
use std::{os::unix::process::CommandExt, process::Command};

#[builtin(
    desc = "suspend the current shell",
    man = "
SYNOPSIS
    suspend

DESCRIPTION
    Suspends the current shell by sending it the SIGTSTP signal,
    returning to the parent process. It can be resumed by sending it SIGCONT."
)]
pub fn suspend(args: &[Str], _shell: &mut Shell<'_>) -> Status {
    signal::kill(Pid::this(), Signal::SIGSTOP).unwrap();
    Status::SUCCESS
}

#[builtin(
    desc = "toggle debug mode (print commands)",
    man = "
SYNOPSIS
    debug on | off

DESCRIPTION
    Turn on or off the feature to print each command executed to stderr (debug mode)."
)]
pub fn debug(args: &[Str], shell: &mut Shell<'_>) -> Status {
    match args.get(1).map(Str::as_str) {
        Some("on") => shell.set_pre_command(Some(Box::new(|_shell, pipeline| {
            // A string representing the command is stored here.
            eprintln!("> {}", pipeline);
        }))),
        Some("off") => shell.set_pre_command(None),
        _ => {
            return Status::bad_argument("debug: the debug builtin requires on or off as argument")
        }
    }
    Status::SUCCESS
}

#[builtin(
    desc = "exit the shell",
    man = "
SYNOPSIS
    exit

DESCRIPTION
    Makes ion exit. The exit status will be that of the last command executed."
)]
pub fn exit(args: &[Str], shell: &mut Shell<'_>) -> Status {
    // Kill all active background tasks before exiting the shell.
    shell.background_send(Signal::SIGTERM).expect("Could not terminate background jobs");
    let exit_code = args
        .get(1)
        .and_then(|status| status.parse::<i32>().ok())
        .unwrap_or_else(|| shell.previous_status().as_os_code());
    std::process::exit(exit_code);
}

#[builtin(
    desc = "replace the shell with the given command",
    man = "
SYNOPSIS
    exec [-ch] [--help] [command [arguments ...]]

DESCRIPTION
    Execute <command>, replacing the shell with the specified program.
    The <arguments> following the command become the arguments to
    <command>.

OPTIONS
    -c  Execute command with an empty environment."
)]
pub fn exec(args: &[Str], _shell: &mut Shell<'_>) -> Status {
    let mut clear_env = false;
    let mut idx = 1;
    for arg in args.iter().skip(1) {
        match &**arg {
            "-c" => clear_env = true,
            _ => break,
        }
        idx += 1;
    }

    match args.get(idx) {
        Some(argument) => {
            let args = if args.len() > idx + 1 { &args[idx + 1..] } else { &[] };
            let mut command = Command::new(argument.as_str());
            command.args(args.iter().map(Str::as_str));
            if clear_env {
                command.env_clear();
            }
            Status::error(format!("ion: exec: {}", command.exec().to_string()))
        }
        None => Status::error("ion: exec: no command provided"),
    }
}
