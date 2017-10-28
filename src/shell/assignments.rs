use super::Shell;
use super::status::*;
use parser::assignments::*;
use shell::history::ShellHistory;
use std::borrow::Cow;
use std::env;
use std::ffi::OsStr;
use std::fmt::{self, Display};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
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

impl VariableStore for Shell {
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
                                    let lhs = self.variables.get_var_or_empty(&key.name);
                                    match math(&lhs, key.kind, operator, &value) {
                                        Ok(value) => self.variables.set_var(&key.name, &value),
                                        Err(why) => {
                                            eprintln!("ion: assignment error: {}", why);
                                            return FAILURE;
                                        }
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
                                let lhs = self.variables.get_var_or_empty(&key.name);
                                match math(&lhs, key.kind, operator, &value) {
                                    Ok(value) => {
                                        let value = OsStr::from_bytes(&value.as_bytes());
                                        env::set_var(&key.name, &value)
                                    }
                                    Err(why) => {
                                        eprintln!("ion: assignment error: {}", why);
                                        return FAILURE;
                                    }
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
            Err(AssignmentError::NoOperator) => for var in expression.split_whitespace() {
                if let Some(value) = self.variables.get_var(var) {
                    env::set_var(var, value);
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

enum MathError {
    RHS,
    LHS,
    Unsupported,
}

impl Display for MathError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MathError::RHS => write!(fmt, "right hand side has invalid type"),
            MathError::LHS => write!(fmt, "left hand side has invalid type"),
            MathError::Unsupported => write!(fmt, "type does not support operation"),
        }
    }
}

fn parse_f64<F: Fn(f64, f64) -> f64>(lhs: &str, rhs: &str, operation: F) -> Result<f64, MathError> {
    lhs.parse::<f64>().map_err(|_| MathError::LHS).and_then(
        |lhs| rhs.parse::<f64>().map_err(|_| MathError::RHS).map(|rhs| operation(lhs, rhs)),
    )
}

fn parse_i64<F: Fn(i64, i64) -> i64>(lhs: &str, rhs: &str, operation: F) -> Result<i64, MathError> {
    lhs.parse::<i64>().map_err(|_| MathError::LHS).and_then(
        |lhs| rhs.parse::<i64>().map_err(|_| MathError::RHS).map(|rhs| operation(lhs, rhs)),
    )
}

fn math<'a>(
    lhs: &str,
    key: Primitive,
    operator: Operator,
    value: &'a str,
) -> Result<Cow<'a, str>, MathError> {
    let value: String = match operator {
        Operator::Add => if Primitive::Any == key || Primitive::Float == key {
            parse_f64(lhs, value, |lhs, rhs| lhs + rhs)?.to_string()
        } else if let Primitive::Integer = key {
            parse_i64(lhs, value, |lhs, rhs| lhs + rhs)?.to_string()
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Divide => {
            if Primitive::Any == key || Primitive::Float == key || Primitive::Integer == key {
                parse_f64(lhs, value, |lhs, rhs| lhs / rhs)?.to_string()
            } else {
                return Err(MathError::Unsupported);
            }
        }
        Operator::IntegerDivide => if Primitive::Any == key || Primitive::Float == key {
            parse_i64(lhs, value, |lhs, rhs| lhs / rhs)?.to_string()
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Subtract => if Primitive::Any == key || Primitive::Float == key {
            parse_f64(lhs, value, |lhs, rhs| lhs - rhs)?.to_string()
        } else if let Primitive::Integer = key {
            parse_i64(lhs, value, |lhs, rhs| lhs - rhs)?.to_string()
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Multiply => if Primitive::Any == key || Primitive::Float == key {
            parse_f64(lhs, value, |lhs, rhs| lhs * rhs)?.to_string()
        } else if let Primitive::Integer = key {
            parse_i64(lhs, value, |lhs, rhs| lhs * rhs)?.to_string()
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Exponent => if Primitive::Any == key || Primitive::Float == key {
            parse_f64(lhs, value, |lhs, rhs| lhs.powf(rhs))?.to_string()
        } else if let Primitive::Integer = key {
            parse_i64(lhs, value, |lhs, rhs| lhs.pow(rhs as u32))?.to_string()
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Equal => {
            return Ok(Cow::Borrowed(value));
        }
    };

    Ok(Cow::Owned(value))
}
