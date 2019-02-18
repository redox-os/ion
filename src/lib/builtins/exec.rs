use crate::{
    builtins::man_pages::{check_help, MAN_EXEC},
    shell::Shell,
    sys::execve,
};
use small;
use std::error::Error;

/// Executes the givent commmand.
pub(crate) fn exec(shell: &mut Shell, args: &[small::String]) -> Result<(), small::String> {
    const CLEAR_ENV: u8 = 1;

    let mut flags = 0u8;
    let mut idx = 0;
    for arg in args.iter() {
        match &**arg {
            "-c" => flags |= CLEAR_ENV,
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
            Err(execve(argument, args, (flags & CLEAR_ENV) == 1).description().into())
        }
        None => Err("no command provided".into()),
    }
}
