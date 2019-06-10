use crate::{
    builtins::man_pages::{check_help, MAN_EXEC},
    shell::Shell,
    sys::execve,
};
use small;
use std::error::Error;

/// Executes the givent commmand.
pub fn exec(shell: &mut Shell<'_>, args: &[small::String]) -> Result<(), small::String> {
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
            shell.prep_for_exit();
            Err(execve(argument, args, clear_env).description().into())
        }
        None => Err("no command provided".into()),
    }
}
