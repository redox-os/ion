
use std::io::{self, Write};

use shell::status::*;
use shell::variables::Variables;
use types::*;

/// Prints all aliases as `Key = Value`. 
pub(crate) fn print_list(list: &VariableContext) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    for (key, value) in list {
        let _ = stdout
            .write(key.as_bytes())
            .and_then(|_| stdout.write_all(b" = "))
            .and_then(|_| stdout.write_all(value.as_bytes()))
            .and_then(|_| stdout.write_all(b"\n"));
    }
}

pub enum Binding {
    InvalidKey(Identifier),
    ListEntries,
    KeyOnly(Identifier),
    KeyValue(Identifier, Value),
}

/// Parse alias as a `(key, value)` tuple.
fn parse_alias(args: &str) -> Binding {
    // Write all the arguments into a single `String`. 
    let mut char_iter = args.chars();

    // Find the key and advance the iterator until the equals operator is found 
    let mut key = "".to_owned();
    let mut found_key = false;
    // Scans through characters until the key is found, then continues to scan until
    // the equals operator is found.
    while let Some(character) = char_iter.next()  {
        match character {
            ' ' if key.is_empty() => (),
            ' ' => {
                found_key = true;
                break;
            }
            '=' if key.is_empty() => (),
            '=' => {
                found_key = true;
                break;
            }
            _  => {
                key.push(character);
                ()
            }
        }
    }

    let key: Identifier = key.into();

    if !found_key && key.is_empty() {
        Binding::ListEntries
    } else {
        let mut value: Value = char_iter.skip_while(|&x| x == ' ').collect();
        value = value.trim_matches(|x| x == '\"' || x == '\'').to_owned();

        if value.is_empty() {
            Binding::KeyOnly(key)
        } else if !is_valid_key(&key) {
            Binding::InvalidKey(key)
        } else {
            Binding::KeyValue(key, value)
        }
    }
}

fn is_valid_key(name: &str) -> bool {
    name.chars().all(|x| x.is_alphanumeric() || x == '_' || x == '+' || x == '-')
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
            vars.aliases.insert(key, value);
        }
        Binding::ListEntries => print_list(&vars.aliases),
        Binding::KeyOnly(key) => {
            eprintln!("ion: please provide value for alias '{}'", key);
            return FAILURE;
        }
    }
    SUCCESS
}

