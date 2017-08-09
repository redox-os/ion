use shell::variables::Variables;
use types::{Array, Identifier, Key, Value as VString};

#[derive(Debug, PartialEq, Clone)]
// TODO: Have the expand_string function return the `Value` type.
pub enum Value {
    String(VString),
    Array(Array),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Binding {
    InvalidKey(Identifier),
    ListEntries,
    KeyOnly(Identifier),
    KeyValue(Identifier, VString),
    MapKeyValue(Identifier, Key, VString),
    Math(Identifier, Operator, VString),
    MultipleKeys(Vec<Identifier>, VString),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Operator {
    Add,
    Subtract,
    Divide,
    Multiply,
    Exponent,
}

#[allow(dead_code)]
enum Expression {
    Arithmetic,
    Regular,
}

/// Parses let bindings, `let VAR = KEY`, returning the result as a `(key, value)` tuple.
pub fn parse_assignment(arguments: &str) -> Binding {
    // Create a character iterator from the arguments.
    let mut char_iter = arguments.chars();

    // Find the key and advance the iterator until the equals operator is found.
    let mut key = "".to_owned();
    let mut keys: Vec<Identifier> = Vec::new();
    let mut found_key = false;
    let mut operator = None;

    macro_rules! match_operator {
        ($op:expr) => {
            if char_iter.next() == Some('=') {
                operator = Some($op);
                found_key = true;
            }
        }
    }

    // Scans through characters until the key is found, then continues to scan until
    // the equals operator is found.
    while let Some(character) = char_iter.next() {
        match character {
            ' ' if key.is_empty() => (),
            ' ' => {
                keys.push(key.clone().into());
                key.clear();
            }
            '+' => {
                match_operator!(Operator::Add);
                break;
            }
            '-' => {
                match_operator!(Operator::Subtract);
                break;
            }
            '*' => {
                match_operator!(Operator::Multiply);
                break;
            }
            '/' => {
                match_operator!(Operator::Divide);
                break;
            }
            '^' => {
                match_operator!(Operator::Exponent);
                break;
            }
            '=' => {
                if !key.is_empty() {
                    keys.push(key.into());
                }
                found_key = true;
                break;
            }
            _ if !found_key => key.push(character),
            _ => (),
        }
    }

    if !found_key {
        Binding::ListEntries
    } else if keys.len() > 1 {
        for key in &keys {
            if !Variables::is_valid_variable_name(&key) {
                return Binding::InvalidKey(key.clone());
            }
        }
        Binding::MultipleKeys(keys, char_iter.skip_while(|&x| x == ' ').collect::<VString>())
    } else if keys.is_empty() {
        Binding::ListEntries
    } else {
        let key = keys.drain(..).next().unwrap();
        let value = char_iter.skip_while(|&x| x == ' ').collect::<VString>();
        if value.is_empty() {
            Binding::KeyOnly(key.into())
        } else if let Some((key, inner_key)) = Variables::is_hashmap_reference(&key) {
            Binding::MapKeyValue(key.into(), inner_key.into(), value)
        } else if !Variables::is_valid_variable_name(&key) {
            Binding::InvalidKey(key.into())
        } else {
            match operator {
                Some(operator) => Binding::Math(key.into(), operator, value),
                None => Binding::KeyValue(key.into(), value),
            }
        }
    }
}
