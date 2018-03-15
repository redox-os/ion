use sys;
use shell::Shell;
use shell::status::*;
use builtins::man_pages::*;

use std::env;
use std::path::Path;

pub(crate) fn which(args: &[&str], shell: &mut Shell) -> Result<i32, String> {
    if check_help(args, MAN_WHICH) {
        return Ok(SUCCESS)
    }

    if args.len() == 1 {
        return Err(String::from("which: Expected at least 1 args, got only 0"))
    }

    let mut result = SUCCESS;
    for &command in &args[1..] {
        let c_type = get_command_info(command, shell);
        if c_type.is_ok() {
            match c_type.unwrap().as_str() {
                "alias" => {
                    let alias = shell.variables.aliases.get(command).unwrap();
                    println!("{}: alias to {}", command, alias);
                },
                "function" => println!("{}: function", command),
                "builtin" => println!("{}: built-in shell command", command),
                _path => println!("{}", _path)
            }
        } else {
            result = FAILURE;
        }
    }
    Ok(result)
}

pub(crate) fn find_type(args: &[&str], shell: &mut Shell) -> Result<i32, String> {
    // Type does not accept help flags, aka "--help".
    if args.len() == 1 {
        return Err(String::from("which: Expected at least 1 args, got only 0"))
    }

    let mut result = FAILURE;
    for &command in &args[1..] {
        let c_type = get_command_info(command, shell);
        if c_type.is_ok() {
            match c_type.unwrap().as_str() {
                "alias" => {
                    let alias = shell.variables.aliases.get(command).unwrap();
                    println!("{} is aliased to `{}`", command, alias);
                },
                // TODO Make it print the function.
                "function" => println!("{} is a function", command), 
                "builtin" => println!("{} is a shell builtin", command),
                _path => println!("{} is {}", command, _path)
            }
            result = SUCCESS;
        } else {
            eprintln!("type: {}: not found", command);
        }
    }
    Ok(result)
}

pub(crate) fn get_command_info(command: &str, shell: &mut Shell) -> Result<String, ()> {
    if shell.variables.aliases.get(command).is_some() {
        return Ok(String::from("alias"))
    } else if shell.functions.contains_key(command) {
        return Ok(String::from("function"))
    } else if shell.builtins.contains_key(command) {
        return Ok(String::from("builtin"))
    } else {
        for path in env::var("PATH")
            .unwrap_or("/bin".to_string())
            .split(sys::PATH_SEPARATOR) 
        {
            let executable = Path::new(path).join(command);
            if executable.is_file() {
                return Ok(executable.display().to_string())
            }
        }
    }
    Err(())
}