use std::io::{self, Write};
use std::env;

use super::variables::Variables;
use super::directory_stack::DirectoryStack;
use parser::assignments::{
    Binding, Operator, Value
};
use parser::{
    ExpanderFunctions,
    Select,
    ArgumentSplitter,
    expand_string,
};
use types::{
    Identifier,
    Value as VString,
    Array as VArray,
    ArrayVariableContext,
    VariableContext
};
use super::status::*;

enum Action {
    UpdateString(Identifier, VString),
    UpdateArray(Identifier, VArray),
    NoOp
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

fn parse_assignment(binding: Binding,
                    vars: &mut Variables,
                    dir_stack: &DirectoryStack) -> Result<Action, i32>
{
    let expanders = get_expanders!(vars, dir_stack);
    match binding {
        Binding::InvalidKey(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: variable name, '{}', is invalid", key);
            Err(FAILURE)
        },
        Binding::KeyValue(key, value) => match parse_expression(&value, &expanders) {
            Value::String(value) => Ok(Action::UpdateString(key, value)),
            Value::Array(array)  => Ok(Action::UpdateArray(key, array))
        },
        Binding::KeyOnly(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: please provide value for variable '{}'", key);
            Err(FAILURE)
        },
        Binding::ListEntries => {
            print_vars(&vars.variables);
            print_arrays(&vars.arrays);
            Ok(Action::NoOp)
        },
        Binding::Math(key, operator, value) => {
            match parse_expression(&value, &expanders) {
                Value::String(ref value) => {
                    let left = match vars.get_var(&key).and_then(|x| x.parse::<f32>().ok()) {
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

pub fn let_assignment(binding: Binding, vars: &mut Variables, dir_stack: &DirectoryStack) -> i32 {
    match parse_assignment(binding, vars, dir_stack) {
        Ok(Action::UpdateArray(key, array)) => vars.set_array(&key, array),
        Ok(Action::UpdateString(key, string)) => vars.set_var(&key, &string),
        Ok(Action::NoOp) => (),
        Err(code) => return code,
    };

    SUCCESS
}

/// Exporting a variable sets that variable as a global variable in the system.
/// Global variables can be accessed by other programs running on the system.
pub fn export_variable(binding : Binding, vars: &mut Variables, dir_stack : &DirectoryStack) -> i32 {
    match parse_assignment(binding, vars, dir_stack) {
        Ok(Action::UpdateArray(key, array)) => env::set_var(&key, array.join(" ")),
        Ok(Action::UpdateString(key, string)) => env::set_var(&key, string),
        Ok(Action::NoOp) => (),
        Err(code) => return code
    };

    SUCCESS
}

fn parse_expression(expression: &str, shell_funcs: &ExpanderFunctions) -> Value {
    let arguments: Vec<&str> = ArgumentSplitter::new(expression).collect();

    if arguments.len() == 1 {
        // If a single argument has been passed, it will be expanded and checked to determine
        // whether or not the expression is an array or a string.
        let expanded = expand_string(expression, shell_funcs, false);
        if expanded.len() == 1 {
            // Grab the inner value and return it as a String.
            Value::String(expanded[0].clone())
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
