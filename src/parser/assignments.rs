use super::arguments::ArgumentSplitter;
use super::shell_expand::{expand_string, ExpanderFunctions};
use variables::Variables;

// TODO: Have the expand_string function return the `Value` type.
pub enum Value {
    String(String),
    Array(Vec<String>)
}

pub enum Binding {
    InvalidKey(String),
    ListEntries,
    KeyOnly(String),
    KeyValue(String, Value),
    Math(String, Operator, Value),
}

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
    Regular
}

fn parse_expression(expression: &str, shell_funcs: &ExpanderFunctions) -> Value {
    let kind = Expression::Regular;

    match kind {
        // TODO: Determine if the expression is an arithmetic expression or not
        Expression::Arithmetic => unimplemented!(),
        // Expands the supplied expression normally
        Expression::Regular => {
            let arguments: Vec<String> = ArgumentSplitter::new(expression).collect();

            if arguments.len() == 1 {
                // If a single argument has been passed, it will be expanded and checked to determine
                // whether or not the expression is an array or a string.
                let mut expanded = expand_string(expression, shell_funcs, false);
                if expanded.len() == 1 {
                    // Grab the inner value and return it as a String.
                    Value::String(expanded.drain(..).next().unwrap())
                } else {
                    // Return the expanded values as an Array.
                    Value::Array(expanded)
                }
            } else {
                // If multiple arguments have been passed, they will be collapsed into a single string.
                // IE: `[ one two three ] four` is equivalent to `one two three four`
                let arguments: Vec<String> = arguments.iter()
                    .flat_map(|expression| expand_string(expression, shell_funcs, false))
                    .collect();

                Value::String(arguments.join(" "))
            }
        }
    }
}

/// Parses let bindings, `let VAR = KEY`, returning the result as a `(key, value)` tuple.
pub fn parse_assignment(arguments: &str, shell_funcs: &ExpanderFunctions) -> Binding {
    // Create a character iterator from the arguments.
    let mut char_iter = arguments.chars();

    // Find the key and advance the iterator until the equals operator is found.
    let mut key = "".to_owned();
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
            ' ' => found_key = true,
            '+' => {
                match_operator!(Operator::Add);
                break
            },
            '-' => {
                match_operator!(Operator::Subtract);
                break
            },
            '*' => {
                match_operator!(Operator::Multiply);
                break
            },
            '/' => {
                match_operator!(Operator::Divide);
                break
            },
            '^' => {
                match_operator!(Operator::Exponent);
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
                Some(operator) => Binding::Math(key, operator, parse_expression(&value, shell_funcs)),
                None => Binding::KeyValue(key, parse_expression(&value, shell_funcs))
            }
        }
    }
}
