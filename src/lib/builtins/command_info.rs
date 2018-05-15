use builtins::man_pages::*;
use shell::{status::*, Shell};
use sys;

use std::{borrow::Cow, env, path::Path};

pub(crate) fn which(args: &[&str], shell: &mut Shell) -> Result<i32, ()> {
    if check_help(args, MAN_WHICH) {
        return Ok(SUCCESS);
    }

    if args.len() == 1 {
        eprintln!("which: Expected at least 1 args, got only 0");
        return Err(());
    }

    let mut result = SUCCESS;
    for &command in &args[1..] {
        if let Ok(c_type) = get_command_info(command, shell) {
            match c_type.as_ref() {
                "alias" => {
                    let alias = shell.variables.aliases.get(command).unwrap();
                    println!("{}: alias to {}", command, alias);
                }
                "function" => println!("{}: function", command),
                "builtin" => println!("{}: built-in shell command", command),
                _path => println!("{}", _path),
            }
        } else {
            result = FAILURE;
        }
    }
    Ok(result)
}

pub(crate) fn find_type(args: &[&str], shell: &mut Shell) -> Result<i32, ()> {
    // Type does not accept help flags, aka "--help".
    if args.len() == 1 {
        eprintln!("type: Expected at least 1 args, got only 0");
        return Err(());
    }

    let mut result = FAILURE;
    for &command in &args[1..] {
        if let Ok(c_type) = get_command_info(command, shell) {
            match c_type.as_ref() {
                "alias" => {
                    let alias = shell.variables.aliases.get(command).unwrap();
                    println!("{} is aliased to `{}`", command, alias);
                }
                // TODO Make it print the function.
                "function" => println!("{} is a function", command),
                "builtin" => println!("{} is a shell builtin", command),
                _path => println!("{} is {}", command, _path),
            }
            result = SUCCESS;
        } else {
            eprintln!("type: {}: not found", command);
        }
    }
    Ok(result)
}

pub(crate) fn get_command_info<'a>(command: &str, shell: &mut Shell) -> Result<Cow<'a, str>, ()> {
    if shell.variables.aliases.get(command).is_some() {
        return Ok("alias".into());
    } else if shell.functions.contains_key(command) {
        return Ok("function".into());
    } else if shell.builtins.contains_key(command) {
        return Ok("builtin".into());
    } else {
        for path in env::var("PATH")
            .unwrap_or("/bin".to_string())
            .split(sys::PATH_SEPARATOR)
        {
            let executable = Path::new(path).join(command);
            if executable.is_file() {
                return Ok(executable.display().to_string().into());
            }
        }
    }
    Err(())
}
