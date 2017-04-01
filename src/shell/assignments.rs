use std::collections::HashMap;
use std::io::{self, Write};

use variables::Variables;
use directory_stack::DirectoryStack;
use parser::assignments::{self, Binding, Operator, Value};
use parser::{ExpanderFunctions, Index, IndexPosition};
use status::*;

fn print_vars(list: &HashMap<String, String>) {
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

fn print_arrays(list: &HashMap<String, Vec<String>>) {
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

pub fn let_assignment<'a>(original: &'a str, vars: &mut Variables, dir_stack: &DirectoryStack) -> i32 {
    let binding = {
        let expanders = ExpanderFunctions {
            tilde: &|tilde: &str| vars.tilde_expansion(tilde, dir_stack),
            array: &|array: &str, index: Index| {
                match vars.get_array(array) {
                    Some(array) => match index {
                            Index::None => None,
                            Index::All => Some(array.to_owned()),
                            Index::ID(id) => array.get(id).map(|x| vec![x.to_owned()]),
                            Index::Range(start, end) => {
                                let array = match end {
                                    IndexPosition::CatchAll => array.iter().skip(start)
                                        .map(|x| x.to_owned()).collect::<Vec<String>>(),
                                    IndexPosition::ID(end) => array.iter().skip(start).take(end-start)
                                        .map(|x| x.to_owned()).collect::<Vec<String>>()
                                };
                                if array.is_empty() { None } else { Some(array) }
                            }
                    },
                    None => None
                }
            },
            variable: &|variable: &str, quoted: bool| {
                if quoted {
                    vars.get_var(variable)
                } else {
                    vars.get_var(variable).map(|x| x.replace("\n", " "))
                }
            },
            command: &|command: &str, quoted: bool| vars.command_expansion(command, quoted),
        };

        assignments::parse_assignment(original, &expanders)
    };

    match binding {
        Binding::InvalidKey(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: variable name, '{}', is invalid", key);
            return FAILURE;
        },
        Binding::KeyValue(key, Value::String(value)) => vars.set_var(&key, &value),
        Binding::KeyValue(key, Value::Array(array))  => vars.set_array(&key, array),
        Binding::KeyOnly(key) => {
            let stderr = io::stderr();
            let _ = writeln!(&mut stderr.lock(), "ion: please provide value for variable '{}'", key);
            return FAILURE;
        },
        Binding::ListEntries => {
            print_vars(&vars.variables);
            print_arrays(&vars.arrays);
        },
        Binding::Math(key, operator, Value::String(value)) => {
            let left = match vars.get_var(&key).and_then(|x| x.parse::<f32>().ok()) {
                Some(left) => left,
                None => return FAILURE,
            };

            let right = match value.parse::<f32>().ok() {
                Some(right) => right,
                None => return FAILURE
            };

            let result = match operator {
                Operator::Add      => left + right,
                Operator::Subtract => left - right,
                Operator::Divide   => left / right,
                Operator::Multiply => left * right,
                Operator::Exponent => f32::powf(left, right)
            };

            vars.set_var(&key, &result.to_string());
        },
        Binding::Math(_, _, Value::Array(_)) => {
            unimplemented!();
        }
    }

    SUCCESS
}
