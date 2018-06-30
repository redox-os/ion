// TODO: Move into grammar

use std::io::{self, Write};

use shell::{status::*, variables::Variables};
use types::*;

fn print_list(vars: &Variables) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    for (key, value) in vars.aliases() {
        let _ = stdout
            .write(key.as_bytes())
            .and_then(|_| stdout.write_all(b" = "))
            .and_then(|_| stdout.write_all(value.as_bytes()))
            .and_then(|_| stdout.write_all(b"\n"));
    }
}

enum Binding {
    InvalidKey(Identifier),
    ListEntries,
    KeyOnly(Identifier),
    KeyValue(Identifier, Value),
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

    let key: Identifier = key.into();

    if !found_key && key.is_empty() {
        Binding::ListEntries
    } else {
        let value: Value = char_iter.skip_while(|&x| x == ' ').collect();
        if value.is_empty() {
            Binding::KeyOnly(key)
        } else if !Variables::is_valid_variable_name(&key) {
            Binding::InvalidKey(key)
        } else {
            Binding::KeyValue(key, value)
        }
    }
}

/// The `alias` command will define an alias for another command, and thus may be used as a
/// command itself.
pub(crate) fn alias(vars: &mut Variables, args: &str) -> i32 {
    match parse_alias(args) {
        Binding::InvalidKey(key) => {
            eprintln!("ion: alias name, '{}', is invalid", key);
            return FAILURE;
        }
        Binding::KeyValue(key, value) => {
            vars.set(&key, Alias(value));
        }
        Binding::ListEntries => print_list(&vars),
        Binding::KeyOnly(key) => {
            eprintln!("ion: please provide value for alias '{}'", key);
            return FAILURE;
        }
    }
    SUCCESS
}

/// Dropping an alias will erase it from the shell.
pub(crate) fn drop_alias<S: AsRef<str>>(vars: &mut Variables, args: &[S]) -> i32 {
    if args.len() <= 1 {
        eprintln!("ion: you must specify an alias name");
        return FAILURE;
    }
    for alias in args.iter().skip(1) {
        if vars.remove_variable(alias.as_ref()).is_none() {
            eprintln!("ion: undefined alias: {}", alias.as_ref());
            return FAILURE;
        }
    }
    SUCCESS
}

/// Dropping an array will erase it from the shell.
pub(crate) fn drop_array<S: AsRef<str>>(vars: &mut Variables, args: &[S]) -> i32 {
    if args.len() <= 2 {
        eprintln!("ion: you must specify an array name");
        return FAILURE;
    }

    if args[1].as_ref() != "-a" {
        eprintln!("ion: drop_array must be used with -a option");
        return FAILURE;
    }

    for array in args.iter().skip(2) {
        if vars.remove_variable(array.as_ref()).is_none() {
            eprintln!("ion: undefined array: {}", array.as_ref());
            return FAILURE;
        }
    }
    SUCCESS
}

/// Dropping a variable will erase it from the shell.
pub(crate) fn drop_variable<S: AsRef<str>>(vars: &mut Variables, args: &[S]) -> i32 {
    if args.len() <= 1 {
        eprintln!("ion: you must specify a variable name");
        return FAILURE;
    }

    for variable in args.iter().skip(1) {
        if vars.remove_variable(variable.as_ref()).is_none() {
            eprintln!("ion: undefined variable: {}", variable.as_ref());
            return FAILURE;
        }
    }

    SUCCESS
}

#[cfg(test)]
mod test {
    use super::*;
    use parser::{expand_string, Expander};
    use shell::status::{FAILURE, SUCCESS};

    struct VariableExpander(pub Variables);

    impl Expander for VariableExpander {
        fn string(&self, var: &str, _: bool) -> Option<Value> { self.0.get::<Value>(var) }
    }

    // TODO: Rewrite tests now that let is part of the grammar.
    // #[test]
    // fn let_and_expand_a_variable() {
    //     let mut variables = Variables::default();
    //     let dir_stack = new_dir_stack();
    //     let_(&mut variables, vec!["let", "FOO", "=", "BAR"]);
    // let expanded = expand_string("$FOO", &variables, &dir_stack,
    // false).join("");     assert_eq!("BAR", &expanded);
    // }
    //
    // #[test]
    // fn let_fails_if_no_value() {
    //     let mut variables = Variables::default();
    //     let return_status = let_(&mut variables, vec!["let", "FOO"]);
    //     assert_eq!(FAILURE, return_status);
    // }
    //
    // #[test]
    // fn let_checks_variable_name() {
    //     let mut variables = Variables::default();
    // let return_status = let_(&mut variables, vec!["let", ",;!:", "=",
    // "FOO"]);     assert_eq!(FAILURE, return_status);
    // }

    #[test]
    fn drop_deletes_variable() {
        let mut variables = Variables::default();
        variables.set("FOO", "BAR".to_string());
        let return_status = drop_variable(&mut variables, &["drop", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = expand_string("$FOO", &VariableExpander(variables), false).join("");
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, &["drop"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, &["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_deletes_array() {
        let mut variables = Variables::default();
        variables.set("FOO", array!["BAR"]);
        let return_status = drop_array(&mut variables, &["drop", "-a", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = expand_string("@FOO", &VariableExpander(variables), false).join("");
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_array_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_array(&mut variables, &["drop", "-a"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_array_fails_with_undefined_array() {
        let mut variables = Variables::default();
        let return_status = drop_array(&mut variables, &["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }
}
