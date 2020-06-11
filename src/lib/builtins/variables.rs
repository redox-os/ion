// TODO: Move into grammar

use std::io::{self, Write};

use super::Status;
use crate as ion_shell;
use crate::{shell::variables::Variables, types, Shell};
use builtins_proc::builtin;

fn print_list(vars: &Variables) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    for (key, value) in vars.aliases() {
        writeln!(stdout, "{} = {}", key, value).unwrap();
    }
}

enum Binding {
    InvalidKey(types::Str),
    ListEntries,
    KeyOnly(types::Str),
    KeyValue(types::Str, types::Str),
}

fn is_valid_alias(name: &str) -> bool {
    let mut iter = name.chars();
    iter.next().map_or(false, |c| c.is_alphabetic() || c == '_')
        && iter.all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '!')
}

/// Parses alias as a `(key, value)` tuple.
fn parse_alias(args: &str) -> Binding {
    // Write all the arguments into a single `String`
    let mut char_iter = args.chars();

    // Find the key and advance the iterator until the equals operator is found.
    let mut key = "".to_owned();
    let mut found_key = false;

    // Scans through characters until the key is found, then continues to scan until
    // the equals operator is found.
    while let Some(character) = char_iter.next() {
        match character {
            ' ' if key.is_empty() => (),
            ' ' => found_key = true,
            '=' => {
                found_key = true;
                break;
            }
            _ if !found_key => key.push(character),
            _ => (),
        }
    }

    let key: types::Str = key.into();

    if !found_key && key.is_empty() {
        Binding::ListEntries
    } else {
        let value: String = char_iter.skip_while(|&x| x == ' ').collect();
        if value.is_empty() {
            Binding::KeyOnly(key)
        } else if is_valid_alias(&key) {
            Binding::KeyValue(key, value.into())
        } else {
            Binding::InvalidKey(key)
        }
    }
}

/// The `alias` command will define an alias for another command, and thus may be used as a
/// command itself.
pub fn builtin_alias(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match parse_alias(&args[1..].join(" ")) {
        Binding::InvalidKey(key) => {
            return Status::error(format!("ion: alias name, '{}', is invalid", key));
        }
        Binding::KeyValue(key, value) => {
            shell.variables_mut().set(&key, types::Alias(value));
        }
        Binding::ListEntries => print_list(shell.variables()),
        Binding::KeyOnly(key) => {
            if let Some(alias) = shell.variables().get(&key) {
                println!("alias {}='{}'", key, alias);
            } else {
                return Status::error(format!("ion: alias '{}' not found", key));
            }
        }
    }
    Status::SUCCESS
}

/// Dropping an alias will erase it from the shell.
pub fn builtin_unalias(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if args.len() <= 1 {
        return Status::error("ion: you must specify an alias name".to_string());
    }
    for alias in args.iter().skip(1) {
        if shell.variables_mut().remove(alias.as_ref()).is_none() {
            return Status::error(format!("ion: undefined alias: {}", alias));
        }
    }
    Status::SUCCESS
}

#[builtin(
    desc = "delete some variables or arrays",
    man = "
SYNOPSIS
    drop VARIABLES...

DESCRIPTION
    Deletes the variables given to it as arguments. The variables name must be supplied.
    Instead of '$x' use 'x'.
"
)]
/// Dropping a variable will erase it from the shell.
pub fn drop(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if args.len() <= 1 {
        return Status::error("ion: you must specify a variable name".to_string());
    }

    for variable in args.iter().skip(1) {
        if shell.variables_mut().remove(variable.as_ref()).is_none() {
            return Status::error(format!("ion: undefined variable: {}", variable));
        }
    }

    Status::SUCCESS
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::expansion::Expander;

    fn vec_string(args: &[&str]) -> Vec<types::Str> { args.iter().map(|s| (*s).into()).collect() }

    // TODO: Rewrite tests now that let is part of the grammar.
    // #[test]
    // fn let_and_expand_a_variable() {
    //     let mut shell = Shell::default();
    //     let dir_stack = new_dir_stack();
    //     let_(&mut variables, vec!["let", "FOO", "=", "BAR"]);
    // let expanded = expand_string("$FOO", &variables, &dir_stack,
    // false).join("");     assert_eq!("BAR", &expanded);
    // }
    //
    // #[test]
    // fn let_fails_if_no_value() {
    //     let mut shell = Shell::default();
    //     let return_status = let_(&mut variables, vec!["let", "FOO"]);
    //     assert_eq!(FAILURE, return_status);
    // }
    //
    // #[test]
    // fn let_checks_variable_name() {
    //     let mut shell = Shell::default();
    // let return_status = let_(&mut variables, vec!["let", ",;!:", "=",
    // "FOO"]);     assert_eq!(FAILURE, return_status);
    // }

    #[test]
    fn drop_deletes_variable() {
        let mut shell = Shell::default();
        shell.variables_mut().set("FOO", "BAR");
        let return_status = builtin_drop(&vec_string(&["drop", "FOO"]), &mut shell);
        assert!(return_status.is_success());
        assert!(shell.expand_string("$FOO").is_err());
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut shell = Shell::default();
        let return_status = builtin_drop(&vec_string(&["drop"]), &mut shell);
        assert!(return_status.is_failure());
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut shell = Shell::default();
        let return_status = builtin_drop(&vec_string(&["drop", "FOO"]), &mut shell);
        assert!(return_status.is_failure());
    }

    #[test]
    fn drop_deletes_array() {
        let mut shell = Shell::default();
        shell.variables_mut().set("FOO", types_rs::array!["BAR"]);
        let return_status = builtin_drop(&vec_string(&["drop", "FOO"]), &mut shell);
        assert_eq!(Status::SUCCESS, return_status);
        assert!(shell.expand_string("@FOO").is_err());
    }

    #[test]
    fn drop_array_fails_with_no_arguments() {
        let mut shell = Shell::default();
        let return_status = builtin_drop(&vec_string(&["drop"]), &mut shell);
        assert!(return_status.is_failure());
    }

    #[test]
    fn drop_array_fails_with_undefined_array() {
        let mut shell = Shell::default();
        let return_status = builtin_drop(&vec_string(&["drop", "FOO"]), &mut shell);
        assert!(return_status.is_failure());
    }
}
