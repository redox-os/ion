use std::env;
use std::io::{self, Write};

use super::Shell;
use super::status::*;
use parser::assignments::*;
use shell::history::ShellHistory;
use types::{ArrayVariableContext, VariableContext};

fn print_vars(list: &VariableContext) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    let _ = stdout.write(b"# Variables\n");
    for (key, value) in list {
        let _ = stdout
            .write(key.as_bytes())
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

/// Represents: A variable store capable of setting local variables or
/// exporting variables to some global environment
pub(crate) trait VariableStore {
    /// Set a local variable given a binding
    fn local(&mut self, &str) -> i32;
    /// Export a variable to the process environment given a binding
    fn export(&mut self, &str) -> i32;
}

impl<'a> VariableStore for Shell<'a> {
    fn local(&mut self, expression: &str) -> i32 {
        match AssignmentActions::new(expression) {
            Ok(assignment_actions) => {
                for action in assignment_actions {
                    match action {
                        Ok(Action::UpdateArray(key, Operator::Equal, expression)) => {
                            match value_check(self, &expression, key.kind) {
                                Ok(ReturnValue::Vector(values)) => {
                                    // When we changed the HISTORY_IGNORE variable, update the
                                    // ignore patterns. This happens first because `set_array`
                                    // consumes 'values'
                                    if key.name == "HISTORY_IGNORE" {
                                        self.update_ignore_patterns(&values);
                                    }
                                    self.variables.set_array(key.name, values)
                                }
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}: {}", key.name, why);
                                    return FAILURE;
                                }
                                _ => unreachable!(),
                            }
                        }
                        Ok(Action::UpdateArray(..)) => {
                            eprintln!(
                                "ion: arithmetic operators on array expressions aren't supported \
                                 yet."
                            );
                            return FAILURE;
                        }
                        Ok(Action::UpdateString(key, operator, expression)) => {
                            if ["HOME", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                                eprintln!("ion: not allowed to set {}", key.name);
                                return FAILURE;
                            }

                            match value_check(self, &expression, key.kind) {
                                Ok(ReturnValue::Str(value)) => {
                                    if !integer_math(self, key, operator, &value) {
                                        return FAILURE;
                                    }
                                }
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}: {}", key.name, why);
                                    return FAILURE;
                                }
                                _ => unreachable!(),
                            }
                        }
                        Err(why) => {
                            eprintln!("ion: assignment error: {}", why);
                            return FAILURE;
                        }
                    }
                }
            }
            Err(AssignmentError::NoKeys) => {
                print_vars(&self.variables.variables);
                print_arrays(&self.variables.arrays);
            }
            Err(why) => {
                eprintln!("ion: assignment error: {}", why);
                return FAILURE;
            }
        }

        SUCCESS
    }

    fn export(&mut self, expression: &str) -> i32 {
        match AssignmentActions::new(expression) {
            Ok(assignment_actions) => for action in assignment_actions {
                match action {
                    Ok(Action::UpdateArray(key, Operator::Equal, expression)) => {
                        match value_check(self, &expression, key.kind) {
                            Ok(ReturnValue::Vector(values)) => {
                                env::set_var(key.name, values.join(" "))
                            }
                            Err(why) => {
                                eprintln!("ion: assignment error: {}: {}", key.name, why);
                                return FAILURE;
                            }
                            _ => unreachable!(),
                        }
                    }
                    Ok(Action::UpdateArray(..)) => {
                        eprintln!(
                            "ion: arithmetic operators on array expressions aren't supported yet."
                        );
                        return FAILURE;
                    }
                    Ok(Action::UpdateString(key, operator, expression)) => {
                        match value_check(self, &expression, key.kind) {
                            Ok(ReturnValue::Str(value)) => {
                                if !integer_math_export(&self, key, operator, &value) {
                                    return FAILURE;
                                }
                            }
                            Err(why) => {
                                eprintln!("ion: assignment error: {}: {}", key.name, why);
                                return FAILURE;
                            }
                            _ => unreachable!(),
                        }
                    }
                    Err(why) => {
                        eprintln!("ion: assignment error: {}", why);
                        return FAILURE;
                    }
                }
            },
            Err(AssignmentError::NoKeys) => {
                let stdout = io::stdout();
                let stdout = &mut stdout.lock();
                for (key, value) in env::vars() {
                    let _ = stdout
                        .write(key.as_bytes())
                        .and_then(|_| stdout.write_all(b"="))
                        .and_then(|_| stdout.write_all(value.as_bytes()))
                        .and_then(|_| stdout.write_all(b"\n"));
                }
            }
            Err(why) => {
                eprintln!("ion: assignment error: {}", why);
                return FAILURE;
            }
        }

        SUCCESS
    }
}

// NOTE: Here there be excessively long functions.

fn integer_math(shell: &mut Shell, key: Key, operator: Operator, value: &str) -> bool {
    match operator {
        Operator::Add => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs + rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs + rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Divide => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs / rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs / rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::IntegerDivide => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs / rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Subtract => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs - rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs - rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Multiply => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs * rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs * rhs).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Exponent => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs.powf(rhs)).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<u32>() {
                    Ok(rhs) => {
                        let value = (lhs.pow(rhs)).to_string();
                        shell.variables.set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Equal => {
            shell.variables.set_var(key.name, &value);
            true
        }
    }
}

fn integer_math_export(shell: &Shell, key: Key, operator: Operator, value: &str) -> bool {
    match operator {
        Operator::Add => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs + rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs + rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Divide => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs / rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs / rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::IntegerDivide => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs / rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Subtract => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs - rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs - rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Multiply => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs * rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<i64>() {
                    Ok(rhs) => {
                        let value = (lhs * rhs).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Exponent => if Primitive::Any == key.kind || Primitive::Float == key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                Ok(lhs) => match value.parse::<f64>() {
                    Ok(rhs) => {
                        let value = (lhs.powf(rhs)).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else if let Primitive::Integer = key.kind {
            match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                Ok(lhs) => match value.parse::<u32>() {
                    Ok(rhs) => {
                        let value = (lhs.pow(rhs)).to_string();
                        env::set_var(key.name, &value);
                        return true;
                    }
                    Err(_) => eprintln!("ion: right hand side has invalid value type"),
                },
                Err(_) => eprintln!("ion: variable has invalid value type"),
            }
            return false;
        } else {
            eprintln!("ion: variable does not support this operation");
            return false;
        },
        Operator::Equal => {
            env::set_var(key.name, &value);
            true
        }
    }
}
