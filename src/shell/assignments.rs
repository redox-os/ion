use std::io::{self, Write};
use std::env;

use parser::assignments::{
    Binding, Operator, Value
};
use parser::{
    Expander,
    ArgumentSplitter,
    expand_string,
};
use types::{
    Identifier,
    Value as VString,
    Array as VArray,
    Key,
    ArrayVariableContext,
    VariableContext,
};
use super::status::*;
use super::Shell;

enum Action {
    UpdateString(Identifier, VString),
    UpdateStrings(Vec<Identifier>, VArray),
    UpdateHashMap(Identifier, Key, VString),
    UpdateArray(Identifier, VArray),
    List
}

fn print_vars(list: &VariableContext) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    let _ = stdout.write(b"# Variables\n");
    for (key, value) in list {
        let _ = stdout.write(key.as_bytes())
            .and_then(|_| stdout.write_all(b" = "))
            .and_then(|_| stdout.write_all(value.as_bytes()))
            .and_then(|_| stdout.write_all(b"\n"));
    }
}

fn print_arrays(list: &ArrayVariableContext) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    let _ = stdout.write(b"\n# Arrays\n");
    for (key, value) in list {
        let _ = stdout.write(key.as_bytes()).and_then(|_| stdout.write_all(b" = [ \""));

        let mut elements = value.iter();

        if let Some(element) = elements.next() {
            let _ = stdout.write_all(element.as_bytes());
        }

        for element in elements {
            let _ = stdout.write_all(b"\" \"").and_then(|_| stdout.write_all(element.as_bytes()));
        }

        let _ = stdout.write(b"\" ]\n");
    }
}

fn parse_assignment<E: Expander>(
    binding: Binding,
    expanders: &E,
) -> Result<Action, i32> {
    match binding {
        Binding::InvalidKey(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: variable name, '{}', is invalid", key);
            Err(FAILURE)
        },
        Binding::KeyValue(key, value) => match parse_expression(&value, expanders) {
            Value::String(value) => Ok(Action::UpdateString(key, value)),
            Value::Array(array) => Ok(Action::UpdateArray(key, array)),
        },
        Binding::MapKeyValue(key, inner_key, value) => {
            Ok(Action::UpdateHashMap(key, inner_key, value))
        },
        Binding::MultipleKeys(keys, value) => match parse_expression(&value, expanders) {
            Value::String(value) => {
                let array = value.split_whitespace().map(String::from)
                    .collect::<VArray>();
                Ok(Action::UpdateStrings(keys, array))
            },
            Value::Array(array)  => Ok(Action::UpdateStrings(keys, array)),
        },
        Binding::KeyOnly(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: please provide value for variable '{}'", key);
            Err(FAILURE)
        },
        Binding::ListEntries => Ok(Action::List),
        Binding::Math(key, operator, value) => {
            match parse_expression(&value, expanders) {
                Value::String(ref value) => {
                    let left = match expanders.variable(&key, false).and_then(|x| {
                        x.parse::<f32>().ok()
                    }) {
                        Some(left) => left,
                        None => return Err(FAILURE),
                    };

                    let right = match value.parse::<f32>().ok() {
                        Some(right) => right,
                        None => return Err(FAILURE)
                    };

                    let result = match operator {
                        Operator::Add      => left + right,
                        Operator::Subtract => left - right,
                        Operator::Divide   => left / right,
                        Operator::Multiply => left * right,
                        Operator::Exponent => f32::powf(left, right)
                    };

                    Ok(Action::UpdateString(key, result.to_string()))
                },
                Value::Array(_) => {
                    let stderr = io::stderr();
                    let _ = writeln!(stderr.lock(), "ion: array math not supported yet");
                    Err(FAILURE)
                }
            }
        },
    }
}

/// Represents: A variable store capable of setting local variables or
/// exporting variables to some global environment
pub trait VariableStore {
    /// Set a local variable given a binding
    fn local(&mut self, Binding) -> i32;
    /// Export a variable to the process environment given a binding
    fn export(&mut self, Binding) -> i32;
}

impl<'a> VariableStore for Shell<'a> {

    fn local(&mut self, binding: Binding) -> i32 {
        match parse_assignment(binding, self) {
            Ok(Action::UpdateArray(key, array)) => self.variables.set_array(&key, array),
            Ok(Action::UpdateString(key, string)) => self.variables.set_var(&key, &string),
            Ok(Action::UpdateStrings(keys, array)) => {
                for (key, value) in keys.iter().zip(array.iter()) {
                    self.variables.set_var(key, value);
                }
            },
            Ok(Action::UpdateHashMap(key, inner_key, value)) => {
                self.variables.set_hashmap_value(&key, &inner_key, &value)
            },
            Ok(Action::List) => {
                print_vars(&self.variables.variables);
                print_arrays(&self.variables.arrays);
            }
            Err(code) => return code,
        };

        SUCCESS
    }

    fn export(&mut self, binding: Binding) -> i32 {
        match parse_assignment(binding, self) {
            Ok(Action::UpdateArray(key, array)) => env::set_var(&key, array.join(" ")),
            Ok(Action::UpdateString(key, string)) => env::set_var(&key, string),
            Ok(Action::UpdateStrings(keys, array)) => {
                for (key, value) in keys.iter().zip(array.iter()) {
                    env::set_var(key, value);
                }
            }
            Ok(Action::UpdateHashMap(key, inner_key, value)) => {
                self.variables.set_hashmap_value(&key, &inner_key, &value)
            },
            Ok(Action::List) => {
                let stdout = io::stdout();
                let stdout = &mut stdout.lock();
                for (key, value) in env::vars() {
                    let _ = stdout.write(key.as_bytes())
                        .and_then(|_| stdout.write_all(b"="))
                        .and_then(|_| stdout.write_all(value.as_bytes()))
                        .and_then(|_| stdout.write_all(b"\n"));
                }
            }
            Err(code) => return code
        };

        SUCCESS
    }

}

fn parse_expression<E: Expander>(
    expression: &str,
    shell_funcs: &E
) -> Value {
    let arguments: Vec<&str> = ArgumentSplitter::new(expression).collect();

    if arguments.len() == 1 {
        let array = expand_string(expression, shell_funcs, false);
        if expression.starts_with('[') && expression.ends_with(']') {
            Value::Array(array)
        } else {
            Value::String(array.join(" "))
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
