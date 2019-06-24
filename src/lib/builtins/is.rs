use super::Status;
use crate as ion_shell;
use crate::{shell::Shell, types};
use builtins_proc::builtin;

// TODO: Add support for multiple name in builtins man
#[builtin(
    desc = "checks if two arguments are the same",
    man = "
SYNOPSIS
    is [ -h | --help ] [not]

DESCRIPTION
    Returns 0 if the two arguments are equal

OPTIONS
    not
        returns 0 if the two arguments are not equal."
)]
pub fn is(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match args.len() {
        4 => {
            if args[1] != "not" {
                return Status::error(format!("Expected 'not' instead found '{}'", args[1]));
            } else if eval_arg(&*args[2], shell) == eval_arg(&*args[3], shell) {
                return Status::error("");
            }
        }
        3 => {
            if eval_arg(&*args[1], shell) != eval_arg(&*args[2], shell) {
                return Status::error("");
            }
        }
        _ => return Status::error("is needs 3 or 4 arguments"),
    }

    Status::SUCCESS
}

fn eval_arg(arg: &str, shell: &mut Shell<'_>) -> types::Str {
    let value = get_var_string(arg, shell);
    if &*value != "" {
        return value;
    }
    arg.into()
}

// On error returns an empty String.
fn get_var_string(name: &str, shell: &mut Shell<'_>) -> types::Str {
    if name.chars().nth(0).unwrap() != '$' {
        return "".into();
    }

    match shell.variables().get_str(&name[1..]) {
        Ok(s) => s,
        Err(why) => {
            eprintln!("{}", why);
            "".into()
        }
    }
}

#[test]
fn test_is() {
    fn vec_string(args: &[&str]) -> Vec<types::Str> { args.iter().map(|&s| s.into()).collect() }
    let mut shell = Shell::default();
    shell.variables_mut().set("x", "value");
    shell.variables_mut().set("y", "0");

    // Four arguments
    assert!(builtin_is(&vec_string(&["is", " ", " ", " "]), &mut shell).is_failure());
    assert!(builtin_is(&vec_string(&["is", "not", " ", " "]), &mut shell).is_failure());
    assert!(builtin_is(&vec_string(&["is", "not", "$x", "$x"]), &mut shell).is_failure());
    assert!(builtin_is(&vec_string(&["is", "not", "2", "1"]), &mut shell).is_success());
    assert!(builtin_is(&vec_string(&["is", "not", "$x", "$y"]), &mut shell).is_success());

    // Three arguments
    assert!(builtin_is(&vec_string(&["is", "1", "2"]), &mut shell).is_failure());
    assert!(builtin_is(&vec_string(&["is", "$x", "$y"]), &mut shell).is_failure());
    assert!(builtin_is(&vec_string(&["is", " ", " "]), &mut shell).is_success());
    assert!(builtin_is(&vec_string(&["is", "$x", "$x"]), &mut shell).is_success());

    // Two arguments
    assert!(builtin_is(&vec_string(&["is", " "]), &mut shell).is_failure());

    // One argument
    assert!(builtin_is(&vec_string(&["is"]), &mut shell).is_failure());
}
