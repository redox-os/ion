// TODO: Move into grammar

use std::io::{self, Write};

use crate::{
    shell::{status::Status, variables::Variables},
    types,
};

fn print_list(vars: &Variables<'_>) {
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
        } else if !Variables::is_valid_variable_name(&key) {
            Binding::InvalidKey(key)
        } else {
            Binding::KeyValue(key, value.into())
        }
    }
}

/// The `alias` command will define an alias for another command, and thus may be used as a
/// command itself.
pub fn alias(vars: &mut Variables<'_>, args: &str) -> Status {
    match parse_alias(args) {
        Binding::InvalidKey(key) => {
            return Status::error(format!("ion: alias name, '{}', is invalid", key));
        }
        Binding::KeyValue(key, value) => {
            vars.set(&key, types::Alias(value));
        }
        Binding::ListEntries => print_list(&vars),
        Binding::KeyOnly(key) => {
            return Status::error(format!("ion: please provide value for alias '{}'", key));
        }
    }
    Status::SUCCESS
}

/// Dropping an alias will erase it from the shell.
pub fn drop_alias<S: AsRef<str>>(vars: &mut Variables<'_>, args: &[S]) -> Status {
    if args.len() <= 1 {
        return Status::error("ion: you must specify an alias name".to_string());
    }
    for alias in args.iter().skip(1) {
        if vars.remove_variable(alias.as_ref()).is_none() {
            return Status::error(format!("ion: undefined alias: {}", alias.as_ref()));
        }
    }
    Status::SUCCESS
}

/// Dropping an array will erase it from the shell.
pub fn drop_array<S: AsRef<str>>(vars: &mut Variables<'_>, args: &[S]) -> Status {
    if args.len() <= 2 {
        return Status::error("ion: you must specify an array name".to_string());
    }

    if args[1].as_ref() != "-a" {
        return Status::error("ion: drop_array must be used with -a option".to_string());
    }

    for array in args.iter().skip(2) {
        if vars.remove_variable(array.as_ref()).is_none() {
            return Status::error(format!("ion: undefined array: {}", array.as_ref()));
        }
    }
    Status::SUCCESS
}

/// Dropping a variable will erase it from the shell.
pub fn drop_variable<S: AsRef<str>>(vars: &mut Variables<'_>, args: &[S]) -> Status {
    if args.len() <= 1 {
        return Status::error("ion: you must specify a variable name".to_string());
    }

    for variable in args.iter().skip(1) {
        if vars.remove_variable(variable.as_ref()).is_none() {
            return Status::error(format!("ion: undefined variable: {}", variable.as_ref()));
        }
    }

    Status::SUCCESS
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parser::Expander;

    struct VariableExpander<'a>(pub Variables<'a>);

    impl<'a> Expander for VariableExpander<'a> {
        fn string(&self, var: &str) -> Option<types::Str> { self.0.get_str(var) }
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
        variables.set("FOO", "BAR");
        let return_status = drop_variable(&mut variables, &["drop", "FOO"]);
        assert!(return_status.is_success());
        let expanded = VariableExpander(variables).expand_string("$FOO").join("");
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, &["drop"]);
        assert!(!return_status.is_success());
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, &["drop", "FOO"]);
        assert!(!return_status.is_success());
    }

    #[test]
    fn drop_deletes_array() {
        let mut variables = Variables::default();
        variables.set("FOO", array!["BAR"]);
        let return_status = drop_array(&mut variables, &["drop", "-a", "FOO"]);
        assert_eq!(Status::SUCCESS, return_status);
        let expanded = VariableExpander(variables).expand_string("@FOO").join("");
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_array_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_array(&mut variables, &["drop", "-a"]);
        assert!(!return_status.is_success());
    }

    #[test]
    fn drop_array_fails_with_undefined_array() {
        let mut variables = Variables::default();
        let return_status = drop_array(&mut variables, &["drop", "FOO"]);
        assert!(!return_status.is_success());
    }
}
