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
    Math(String, Operator, f32),
    MathInvalid(String)
}

enum Operator {
    Plus,
    Minus,
    Divide,
    Multiply
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
    let mut operator = None;

    // Scans through characters until the key is found, then continues to scan until
    // the equals operator is found.
    while let Some(character) = char_iter.next() {
        match character {
            ' ' if key.is_empty() => (),
            ' ' => found_key = true,
            '+' => {
                if char_iter.next() == Some('=') {
                    operator = Some(Operator::Plus);
                    found_key = true;
                }
                break
            },
            '-' => {
                if char_iter.next() == Some('=') {
                    operator = Some(Operator::Minus);
                    found_key = true;
                }
                break
            },
            '*' => {
                if char_iter.next() == Some('=') {
                    operator = Some(Operator::Multiply);
                    found_key = true;
                }
                break
            },
            '/' => {
                if char_iter.next() == Some('=') {
                    operator = Some(Operator::Divide);
                    found_key = true;
                }
                break
            },
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
            match operator {
                Some(operator) => {
                    match value.parse::<f32>() {
                        Ok(value) => Binding::Math(key, operator, value),
                        Err(_)    => Binding::MathInvalid(value)
                    }
                },
                None => Binding::KeyValue(key, value)
            }
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
        },
        _ => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: invalid alias syntax");
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
        },
        Binding::Math(key, operator, increment) => {
            let value = vars.get_var_or_empty(&key);
            let _ = match value.parse::<f32>() {
                Ok(old_value) => match operator {
                    Operator::Plus     => vars.variables.insert(key, (old_value + increment).to_string()),
                    Operator::Minus    => vars.variables.insert(key, (old_value - increment).to_string()),
                    Operator::Multiply => vars.variables.insert(key, (old_value * increment).to_string()),
                    Operator::Divide   => vars.variables.insert(key, (old_value / increment).to_string()),
                },
                Err(_) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: original value, {}, is not a number", value);
                    return FAILURE;
                }
            };
        },
        Binding::MathInvalid(value) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: supplied value, {}, is not a number", value);
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
        Binding::Math(key, operator, increment) => {
            let value = vars.get_var(&key).unwrap_or_else(|| "".to_owned());
            match value.parse::<f32>() {
                Ok(old_value) => match operator {
                    Operator::Plus     => env::set_var(key, (old_value + increment).to_string()),
                    Operator::Minus    => env::set_var(key, (old_value - increment).to_string()),
                    Operator::Multiply => env::set_var(key, (old_value * increment).to_string()),
                    Operator::Divide   => env::set_var(key, (old_value / increment).to_string()),
                },
                Err(_) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: original value, {}, is not a number", value);
                    return FAILURE;
                }
            }
        },
        Binding::MathInvalid(value) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: supplied value, {}, is not a number", value);
            return FAILURE;
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
        let expanded = expand_string("$FOO", &variables, &dir_stack).join("");
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
        let expanded = expand_string("$FOO", &variables, &new_dir_stack()).join("");
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
