use crate::{
    builtins::man_pages::*,
    shell::{flow_control::Function, status::*, Shell},
    sys, types,
};
use small;

use std::{borrow::Cow, env, path::Path};

pub(crate) fn which(args: &[small::String], shell: &mut Shell) -> Result<i32, ()> {
    if check_help(args, MAN_WHICH) {
        return Ok(SUCCESS);
    }

    if args.len() == 1 {
        eprintln!("which: Expected at least 1 args, got only 0");
        return Err(());
    }

    let mut result = SUCCESS;
    for command in &args[1..] {
        match get_command_info(command, shell) {
            Ok(c_type) => match c_type.as_ref() {
                "alias" => {
                    if let Some(alias) = shell.variables.get::<types::Alias>(&**command) {
                        println!("{}: alias to {}", command, &*alias);
                    }
                }
                "function" => println!("{}: function", command),
                "builtin" => println!("{}: built-in shell command", command),
                _path => println!("{}", _path),
            },
            Err(_) => result = FAILURE,
        }
    }
    Ok(result)
}

pub(crate) fn find_type(args: &[small::String], shell: &mut Shell) -> Result<i32, ()> {
    // Type does not accept help flags, aka "--help".
    if args.len() == 1 {
        eprintln!("type: Expected at least 1 args, got only 0");
        return Err(());
    }

    let mut result = FAILURE;
    for command in &args[1..] {
        match get_command_info(command, shell) {
            Ok(c_type) => {
                match c_type.as_ref() {
                    "alias" => {
                        if let Some(alias) = shell.variables.get::<types::Alias>(&**command) {
                            println!("{} is aliased to `{}`", command, &*alias);
                        }
                    }
                    // TODO Make it print the function.
                    "function" => println!("{} is a function", command),
                    "builtin" => println!("{} is a shell builtin", command),
                    _path => println!("{} is {}", command, _path),
                }
                result = SUCCESS;
            }
            Err(_) => eprintln!("type: {}: not found", command),
        }
    }
    Ok(result)
}

pub(crate) fn get_command_info<'a>(command: &str, shell: &mut Shell) -> Result<Cow<'a, str>, ()> {
    if shell.variables.get::<types::Alias>(command).is_some() {
        return Ok("alias".into());
    } else if shell.variables.get::<Function>(command).is_some() {
        return Ok("function".into());
    } else if shell.builtins().contains_key(command) {
        return Ok("builtin".into());
    } else {
        for path in
            env::var("PATH").unwrap_or_else(|_| String::from("/bin")).split(sys::PATH_SEPARATOR)
        {
            let executable = Path::new(path).join(command);
            if executable.is_file() {
                return Ok(executable.display().to_string().into());
            }
        }
    }
    Err(())
}
