use super::{
    flow_control::{ExportAction, LocalAction}, status::*, Shell,
};
use itoa;
use parser::assignments::*;
use shell::history::ShellHistory;
use std::{
    env, ffi::OsStr, fmt::{self, Display}, io::{self, BufWriter, Write}, mem,
    os::unix::ffi::OsStrExt, str,
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
                vars.for_each(|ref var| {
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
    shell.variables.variables.iter().for_each(|(key, val)| {
        let _ = buffer.write([key, " = ", val.as_str(), "\n"].concat().as_bytes());
    });

    // Then immediately follow that with a list of array variables.
    let _ = buffer.write(b"\n# Array Variables\n");
    shell
        .variables
        .arrays
        .iter()
        .for_each(|(key, val)| print_array(&mut buffer, &key, &val));
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
                    match value_check(self, &expression, key.kind) {
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
                    match value_check(self, &expression, key.kind) {
                        Ok(ReturnValue::Str(value)) => {
                            let key_name: &str = &key.name;
                            let lhs = self
                                .variables
                                .variables
                                .get(key_name)
                                .map(|x| x.as_str())
                                .unwrap_or("0");

                            let result = math(&lhs, key.kind, operator, &value, |value| {
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
        let actions = match action {
            LocalAction::List => {
                list_vars(&self);
                return SUCCESS;
            }
            LocalAction::Assign(ref keys, op, ref vals) => AssignmentActions::new(keys, op, vals),
        };
        for action in actions {
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
                        "ion: arithmetic operators on array expressions aren't supported yet."
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
                            let key_name: &str = &key.name;
                            let lhs = self
                                .variables
                                .variables
                                .get(key_name)
                                .map(|x| x.as_str())
                                .unwrap_or("0") as *const str;

                            let result =
                                math(unsafe { &*lhs }, key.kind, operator, &value, |value| {
                                    self.set_var(key_name, unsafe {
                                        str::from_utf8_unchecked(value)
                                    })
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
    key: Primitive,
    operator: Operator,
    value: &'a str,
    mut writefn: F,
) -> Result<(), MathError> {
    match operator {
        Operator::Add => if Primitive::Any == key || Primitive::Float == key {
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
            if Primitive::Any == key || Primitive::Float == key || Primitive::Integer == key {
                writefn(
                    parse_f64(lhs, value, |lhs, rhs| lhs / rhs)?
                        .to_string()
                        .as_bytes(),
                );
            } else {
                return Err(MathError::Unsupported);
            }
        }
        Operator::IntegerDivide => if Primitive::Any == key || Primitive::Float == key {
            write_integer(parse_i64(lhs, value, |lhs, rhs| lhs / rhs)?, writefn);
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Subtract => if Primitive::Any == key || Primitive::Float == key {
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
        Operator::Multiply => if Primitive::Any == key || Primitive::Float == key {
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
        Operator::Exponent => if Primitive::Any == key || Primitive::Float == key {
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
