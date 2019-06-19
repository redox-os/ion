use ion_shell::{
    builtins::{man_pages::check_help, Status},
    sys::SIGTERM,
    types::Str,
    Shell,
};
use std::{error::Error, process::Command};

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
pub fn _exec(args: &[Str]) -> Result<(), Str> {
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
            let mut command = Command::new(argument.as_str());
            command.args(args.iter().map(Str::as_str));
            if clear_env {
                command.env_clear();
            }
            command.spawn().map(|_| ()).map_err(|err| err.description().into())
        }
        None => Err("no command provided".into()),
    }
}

pub fn exit(args: &[Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_EXIT) {
        return Status::SUCCESS;
    }
    // Kill all active background tasks before exiting the shell.
    shell.background_send(SIGTERM);
    let exit_code = args
        .get(1)
        .and_then(|status| status.parse::<i32>().ok())
        .unwrap_or_else(|| shell.previous_status().as_os_code());
    std::process::exit(exit_code);
}

pub fn exec(args: &[Str], _shell: &mut Shell<'_>) -> Status {
    match _exec(&args[1..]) {
        // Shouldn't ever hit this case.
        Ok(()) => unreachable!(),
        Err(err) => Status::error(format!("ion: exec: {}", err)),
    }
}
