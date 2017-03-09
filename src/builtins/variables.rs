use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write};

use status::*;
use variables::Variables;

fn print_list(list: &BTreeMap<String, String>) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    for (key, value) in list {
        let _ = stdout.write(key.as_bytes())
            .and_then(|_| stdout.write_all(b" = "))
            .and_then(|_| stdout.write_all(value.as_bytes()))
            .and_then(|_| stdout.write_all(b"\n"));
    }
}

enum Binding {
    InvalidKey(String),
    ListEntries,
    KeyOnly(String),
    KeyValue(String, String),
}

/// Parses let bindings, `let VAR = KEY`, returning the result as a `(key, value)` tuple.
fn parse_assignment<I: IntoIterator>(args: I)
    -> Binding where I::Item: AsRef<str>
{
    // Write all the arguments into a single `String`
    let arguments = args.into_iter().skip(1).fold(String::new(), |a, b| a + " " + b.as_ref());

    // Create a character iterator from the arguments.
    let mut char_iter = arguments.chars();

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
                break
            },
            _ if !found_key => key.push(character),
            _ => ()
        }
    }

    if !found_key && key.is_empty() {
        Binding::ListEntries
    } else {
        let value = char_iter.skip_while(|&x| x == ' ').collect::<String>();
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
pub fn alias<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
    where I::Item: AsRef<str>
{
    match parse_assignment(args) {
        Binding::InvalidKey(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: alias name, '{}', is invalid", key);
            return FAILURE;
        },
        Binding::KeyValue(key, value) => { vars.aliases.insert(key, value); },
        Binding::ListEntries => print_list(&vars.aliases),
        Binding::KeyOnly(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: please provide value for alias '{}'", key);
            return FAILURE;
        }
    }
    SUCCESS
}


/// Dropping an alias will erase it from the shell.
pub fn drop_alias<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
    where I::Item: AsRef<str>
{
    let args = args.into_iter().collect::<Vec<I::Item>>();
    if args.len() <= 1 {
        let stderr = io::stderr();
        let _ = writeln!(&mut stderr.lock(), "ion: you must specify an alias name");
        return FAILURE;
    }
    for alias in args.iter().skip(1) {
        if vars.aliases.remove(alias.as_ref()).is_none() {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: undefined alias: {}", alias.as_ref());
            return FAILURE;
        }
    }
    SUCCESS
}

/// The `let` command will set a variable within the shell. This variable is only accessible by
/// the shell that created the variable, and other programs may not access it.
pub fn let_<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
    where I::Item: AsRef<str>
{
    match parse_assignment(args) {
        Binding::InvalidKey(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: variable name, '{}', is invalid", key);
            return FAILURE;
        },
        Binding::KeyValue(key, value) => { vars.variables.insert(key, value); },
        Binding::ListEntries => print_list(&vars.variables),
        Binding::KeyOnly(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: please provide value for variable '{}'", key);
            return FAILURE;
        }
    }
    SUCCESS
}

/// Dropping a variable will erase it from the shell.
pub fn drop_variable<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
    where I::Item: AsRef<str>
{
    let args = args.into_iter().collect::<Vec<I::Item>>();
    if args.len() <= 1 {
        let stderr = io::stderr();
        let _ = writeln!(&mut stderr.lock(), "ion: you must specify a variable name");
        return FAILURE;
    }
    for variable in args.iter().skip(1) {
        if vars.unset_var(variable.as_ref()).is_none() {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: undefined variable: {}", variable.as_ref());
            return FAILURE;
        }
    }
    SUCCESS
}


/// Exporting a variable sets that variable as a global variable in the system.
/// Global variables can be accessed by other programs running on the system.
pub fn export_variable<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
    where I::Item: AsRef<str>
{
    match parse_assignment(args) {
        Binding::InvalidKey(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: variable name, '{}', is invalid", key);
            return FAILURE
        },
        Binding::KeyValue(key, value) => env::set_var(key, value),
        Binding::KeyOnly(key) => {
            if let Some(local_value) = vars.get_var(&key) {
                env::set_var(key, local_value);
            } else {
                let stderr = io::stderr();
                let _ = writeln!(&mut stderr.lock(), "ion: unknown variable, '{}'", key);
                return FAILURE;
            }
        },
        _ => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion usage: export KEY=VALUE");
            return FAILURE;
        }
    }
    SUCCESS
}

#[cfg(test)]
mod test {
    use super::*;
    use parser::expand_string;
    use status::{FAILURE, SUCCESS};
    use directory_stack::DirectoryStack;

    fn new_dir_stack() -> DirectoryStack {
        DirectoryStack::new().unwrap()
    }

    #[test]
    fn let_and_expand_a_variable() {
        let mut variables = Variables::default();
        let dir_stack = new_dir_stack();
        let_(&mut variables, vec!["let", "FOO", "=", "BAR"]);
        let expanded = expand_string("$FOO", &variables, &dir_stack).unwrap();
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn let_fails_if_no_value() {
        let mut variables = Variables::default();
        let return_status = let_(&mut variables, vec!["let", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn let_checks_variable_name() {
        let mut variables = Variables::default();
        let return_status = let_(&mut variables, vec!["let", ",;!:", "=", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_deletes_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let return_status = drop_variable(&mut variables, vec!["drop", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = expand_string("$FOO", &variables, &new_dir_stack()).unwrap();
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, vec!["drop"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, vec!["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }
}
