use super::{
    flow_control::{ExportAction, LocalAction}, status::*, Shell,
};
use itoa;
use parser::assignments::*;
use smallvec::SmallVec;
use shell::{
    history::ShellHistory,
    variables::VariableType
};
use std::{
    collections::HashMap,
    env, ffi::OsStr, fmt::{self, Display}, io::{self, BufWriter, Write}, mem,
    os::unix::ffi::OsStrExt, str, simd,
};

fn list_vars(shell: &Shell) {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    // Small function for formatting and append an array entry to a string buffer.
    fn print_array<W: Write>(buffer: &mut W, key: &str, array: &[String]) {
        let _ = buffer.write([key, " = [ "].concat().as_bytes());
        if array.len() > 1 {
            let mut vars = array.iter();
            if let Some(ref var) = vars.next() {
                let _ = buffer.write(["'", var, "', "].concat().as_bytes());
                vars.for_each(|var| {
                    let _ = buffer.write(["'", var, "' "].concat().as_bytes());
                });
            }
            let _ = buffer.write(b"]\n");
        } else {
            let _ = buffer.write(["'", &array[0], "' ]\n"].concat().as_bytes());
        }
    }

    // Write all the string variables to the buffer.
    let _ = buffer.write(b"# String Variables\n");
    for (key, val) in shell.variables.variables() {
        let _ = buffer.write([key, " = ", val.as_str(), "\n"].concat().as_bytes());
    }

    // Then immediately follow that with a list of array variables.
    let _ = buffer.write(b"\n# Array Variables\n");
    for (key, val) in shell.variables.arrays() {
        print_array(&mut buffer, &key, &val)
    }
}

/// Represents: A variable store capable of setting local variables or
/// exporting variables to some global environment
pub(crate) trait VariableStore {
    /// Set a local variable given a binding
    fn local(&mut self, LocalAction) -> i32;
    /// Export a variable to the process environment given a binding
    fn export(&mut self, ExportAction) -> i32;
}

impl VariableStore for Shell {
    fn export(&mut self, action: ExportAction) -> i32 {
        let actions = match action {
            ExportAction::Assign(ref keys, op, ref vals) => AssignmentActions::new(keys, op, vals),
            ExportAction::LocalExport(ref key) => match self.get_var(key) {
                Some(var) => {
                    env::set_var(key, &var);
                    return SUCCESS;
                }
                None => {
                    eprintln!("ion: cannot export {} because it does not exist.", key);
                    return FAILURE;
                }
            },
            ExportAction::List => {
                let stdout = io::stdout();
                let mut stdout = stdout.lock();
                for (key, val) in env::vars() {
                    let _ = writeln!(stdout, "{} =\"{}\"", key, val);
                }
                return SUCCESS;
            }
        };

        for action in actions {
            match action {
                Ok(Action::UpdateArray(key, Operator::Equal, expression)) => {
                    match value_check(self, &expression, &key.kind) {
                        Ok(ReturnValue::Vector(values)) => env::set_var(key.name, values.join(" ")),
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
                    match value_check(self, &expression, &key.kind) {
                        Ok(ReturnValue::Str(value)) => {
                            let key_name: &str = &key.name;
                            let lhs = self
                                .variables
                                .get_var(key_name)
                                .unwrap_or_else(|| String::from("0"));

                            let result = math(&lhs, &key.kind, operator, &value, |value| {
                                env::set_var(key_name, &OsStr::from_bytes(value))
                            });

                            if let Err(why) = result {
                                eprintln!("ion: assignment error: {}", why);
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

        SUCCESS
    }

    fn local(&mut self, action: LocalAction) -> i32 {
        let mut collected: HashMap<&str, ReturnValue> = HashMap::new();
        let (actions_step1, actions_step2) = match action {
            LocalAction::List => {
                list_vars(&self);
                return SUCCESS;
            }
            LocalAction::Assign(ref keys, op, ref vals) => (AssignmentActions::new(keys, op, vals), AssignmentActions::new(keys, op, vals)),
        };
        for action in actions_step1 {
            match action {
                Ok(Action::UpdateArray(key, Operator::Equal, expression)) => {
                    match value_check(self, &expression, &key.kind) {
                        Ok(ReturnValue::Vector(values)) => {
                            // When we changed the HISTORY_IGNORE variable, update the
                            // ignore patterns. This happens first because `set_array`
                            // consumes 'values'
                            if key.name == "HISTORY_IGNORE" {
                                self.update_ignore_patterns(&values);
                            }
                            collected.insert(key.name, ReturnValue::Vector(values));
                        }
                        Ok(ReturnValue::Str(value)) => {
                            collected.insert(key.name, ReturnValue::Str(value));
                        }
                        Err(why) => {
                            eprintln!("ion: assignment error: {}: {}", key.name, why);
                            return FAILURE;
                        }
                    }
                }
                Ok(Action::UpdateArray(..)) => {
                    eprintln!(
                        "ion: arithmetic operators on array expressions aren't supported yet."
                    );
                    return FAILURE;
                }
                Ok(Action::UpdateString(key, operator, expression)) => {
                    if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                        eprintln!("ion: not allowed to set {}", key.name);
                        return FAILURE;
                    }

                    match value_check(self, &expression, &key.kind) {
                        Ok(ReturnValue::Str(value)) => {
                            if operator == Operator::Equal {
                                collected.insert(key.name, ReturnValue::Str(value));
                                continue;
                            }
                            match self.variables.lookup_any(key.name) {
                                Some(VariableType::Variable(lhs)) => {
                                    let result = math(&lhs, &key.kind, operator, &value, |value| {
                                        collected.insert(key.name, ReturnValue::Str(unsafe {
                                            str::from_utf8_unchecked(value)
                                        }.to_owned()));
                                    });

                                    if let Err(why) = result {
                                        eprintln!("ion: assignment error: {}", why);
                                        return FAILURE;
                                    }
                                },
                                Some(VariableType::Array(array)) => {
                                    let mut output = SmallVec::with_capacity(array.len());

                                    let value = match value.parse::<f64>() {
                                        Ok(n) => n,
                                        Err(_) => {
                                            eprintln!("ion: assignment error: value is not a float");
                                            return FAILURE;
                                        }
                                    };

                                    for part in array.chunks(8) {
                                        let mut vec = simd::f64x8::splat(0.0);

                                        for (i, value) in part.iter().enumerate() {
                                            vec = vec.replace(i, match value.parse::<f64>() {
                                                Ok(n) => n,
                                                Err(_) => {
                                                    eprintln!("ion: assignment error: array item is not a float");
                                                    return FAILURE;
                                                }
                                            });
                                        }

                                        match operator {
                                            Operator::Add => vec += value,
                                            Operator::Divide => vec /= value,
                                            Operator::Subtract => vec -= value,
                                            Operator::Multiply => vec *= value,
                                            _ => {
                                                eprintln!("ion: assignment error: operator does not work on arrays");
                                                return FAILURE;
                                            }
                                        }

                                        for i in 0..part.len() {
                                            output.push(vec.extract(i).to_string());
                                        }
                                    }

                                    collected.insert(key.name, ReturnValue::Vector(output));
                                },
                                _ => {
                                    eprintln!("ion: assignment error: type does not support this operator");
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

        for action in actions_step2 {
            match action {
                Ok(Action::UpdateArray(key, _, _)) => {
                    match collected.remove(key.name) {
                        Some(ReturnValue::Vector(values)) => {
                            if let Primitive::Indexed(_, _) = key.kind {
                                eprintln!("ion: multi-dimensional arrays are not yet supported");
                                return FAILURE;
                            } else {
                                self.variables.set_array(key.name, values);
                            }
                        }
                        Some(ReturnValue::Str(value)) => {
                            if let Primitive::Indexed(ref index, _) = key.kind {
                                match parse_index(index, &self) {
                                    Ok(index_num) => self.variables.set_at_array_index(key.name, index_num, &value),
                                    Err(why) => {
                                        eprintln!("{}", why);
                                        return FAILURE;
                                    }
                                };
                            }
                        }
                        _ => ()
                    }
                }
                Ok(Action::UpdateString(key, _, _)) => {
                    match collected.remove(key.name) {
                        Some(ReturnValue::Str(value)) => self.set_var(key.name, &value),
                        Some(ReturnValue::Vector(values)) => self.variables.set_array(key.name, values),
                        _ => ()
                    }
                }
                _ => unreachable!(),
            }
        }

        SUCCESS
    }
}

fn parse_index(index: &str, shell: &Shell) -> Result<usize, String> {
    if index.chars().all(char::is_numeric) {
        Ok(index.parse::<usize>().unwrap()) // It's okay to unwrap since all chars are numeric
    } else if index.starts_with("$") && index.chars().nth(1).map(char::is_alphabetic).unwrap_or(false) {
        match shell.variables.get_var(&index[1..]) {
            Some(value) => {
                match value.parse::<usize>() {
                    Ok(index) => Ok(index),
                    Err(_) => Err(format!("ion: index variable does not contain a numeric value: {}", index)),
                }
            }
            None => Err(format!("ion: index variable does not exist: {}", index)),
        }
    } else {
        Err(format!("ion: invalid array index: {}", index))
    }
}

#[derive(Debug)]
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
    lhs.parse::<f64>()
        .map_err(|_| MathError::LHS)
        .and_then(|lhs| {
            rhs.parse::<f64>()
                .map_err(|_| MathError::RHS)
                .map(|rhs| operation(lhs, rhs))
        })
}

fn parse_i64<F: Fn(i64, i64) -> i64>(lhs: &str, rhs: &str, operation: F) -> Result<i64, MathError> {
    lhs.parse::<i64>()
        .map_err(|_| MathError::LHS)
        .and_then(|lhs| {
            rhs.parse::<i64>()
                .map_err(|_| MathError::RHS)
                .map(|rhs| operation(lhs, rhs))
        })
}

fn write_integer<F: FnMut(&[u8])>(integer: i64, mut func: F) {
    let mut buffer: [u8; 20] = unsafe { mem::uninitialized() };
    let capacity = itoa::write(&mut buffer[..], integer).unwrap();
    func(&buffer[..capacity]);
}

fn math<'a, F: FnMut(&[u8])>(
    lhs: &str,
    key: &Primitive,
    operator: Operator,
    value: &'a str,
    mut writefn: F,
) -> Result<(), MathError> {
    match operator {
        Operator::Add => if Primitive::Any == *key || Primitive::Float == *key {
            writefn(
                parse_f64(lhs, value, |lhs, rhs| lhs + rhs)?
                    .to_string()
                    .as_bytes(),
            );
        } else if let Primitive::Integer = key {
            write_integer(parse_i64(lhs, value, |lhs, rhs| lhs + rhs)?, writefn);
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Divide => {
            if Primitive::Any == *key || Primitive::Float == *key || Primitive::Integer == *key {
                writefn(
                    parse_f64(lhs, value, |lhs, rhs| lhs / rhs)?
                        .to_string()
                        .as_bytes(),
                );
            } else {
                return Err(MathError::Unsupported);
            }
        }
        Operator::IntegerDivide => if Primitive::Any == *key || Primitive::Float == *key {
            write_integer(parse_i64(lhs, value, |lhs, rhs| lhs / rhs)?, writefn);
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Subtract => if Primitive::Any == *key || Primitive::Float == *key {
            writefn(
                parse_f64(lhs, value, |lhs, rhs| lhs - rhs)?
                    .to_string()
                    .as_bytes(),
            );
        } else if let Primitive::Integer = key {
            write_integer(parse_i64(lhs, value, |lhs, rhs| lhs - rhs)?, writefn);
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Multiply => if Primitive::Any == *key || Primitive::Float == *key {
            writefn(
                parse_f64(lhs, value, |lhs, rhs| lhs * rhs)?
                    .to_string()
                    .as_bytes(),
            );
        } else if let Primitive::Integer = key {
            write_integer(parse_i64(lhs, value, |lhs, rhs| lhs * rhs)?, writefn);
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Exponent => if Primitive::Any == *key || Primitive::Float == *key {
            writefn(
                parse_f64(lhs, value, |lhs, rhs| lhs.powf(rhs))?
                    .to_string()
                    .as_bytes(),
            );
        } else if let Primitive::Integer = key {
            write_integer(
                parse_i64(lhs, value, |lhs, rhs| lhs.pow(rhs as u32))?,
                writefn,
            );
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Equal => writefn(value.as_bytes()),
    };

    Ok(())
}
