use crate::{
    builtins::man_pages::*,
    shell::{status::Status, Shell, Value},
    sys, types,
};

use std::{borrow::Cow, env, path::Path};

pub fn which(args: &[types::Str], shell: &mut Shell<'_>) -> Result<Status, ()> {
    if check_help(args, MAN_WHICH) {
        return Ok(Status::SUCCESS);
    }

    if args.len() == 1 {
        eprintln!("which: Expected at least 1 args, got only 0");
        return Err(());
    }

    let mut result = Status::SUCCESS;
    for command in &args[1..] {
        match get_command_info(command, shell) {
            Ok(c_type) => match c_type.as_ref() {
                "alias" => {
                    if let Some(Value::Alias(ref alias)) = shell.variables().get_ref(&**command) {
                        println!("{}: alias to {}", command, &**alias);
                    }
                }
                "function" => println!("{}: function", command),
                "builtin" => println!("{}: built-in shell command", command),
                _path => println!("{}", _path),
            },
            Err(_) => result = Status::from_exit_code(1),
        }
    }
    Ok(result)
}

pub fn find_type(args: &[types::Str], shell: &mut Shell<'_>) -> Result<Status, ()> {
    // Type does not accept help flags, aka "--help".
    if args.len() == 1 {
        eprintln!("type: Expected at least 1 args, got only 0");
        return Err(());
    }

    let mut result = Status::SUCCESS;
    for command in &args[1..] {
        match get_command_info(command, shell) {
            Ok(c_type) => {
                match c_type.as_ref() {
                    "alias" => {
                        if let Some(Value::Alias(alias)) = shell.variables().get_ref(&**command) {
                            println!("{} is aliased to `{}`", command, &**alias);
                        }
                    }
                    // TODO Make it print the function.
                    "function" => println!("{} is a function", command),
                    "builtin" => println!("{} is a shell builtin", command),
                    _path => println!("{} is {}", command, _path),
                }
            }
            Err(_) => result = Status::error(format!("type: {}: not found", command)),
        }
    }
    Ok(result)
}

fn get_command_info<'a>(command: &str, shell: &mut Shell<'_>) -> Result<Cow<'a, str>, ()> {
    match shell.variables().get_ref(command) {
        Some(Value::Alias(_)) => Ok("alias".into()),
        Some(Value::Function(_)) => Ok("function".into()),
        _ if shell.builtins().contains(command) => Ok("builtin".into()),
        _ => {
            for path in
                env::var("PATH").unwrap_or_else(|_| String::from("/bin")).split(sys::PATH_SEPARATOR)
            {
                let executable = Path::new(path).join(command);
                if executable.is_file() {
                    return Ok(executable.display().to_string().into());
                }
            }
            Err(())
        }
    }
}
