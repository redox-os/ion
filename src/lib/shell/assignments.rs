use super::{
    flow_control::{ExportAction, LocalAction},
    status::*,
    Shell,
};
use hashbrown::HashMap;
use itoa;
use lexers::assignments::{Operator, Primitive};
use parser::assignments::*;
use shell::{history::ShellHistory, variables::VariableType};
use std::{
    env,
    ffi::OsStr,
    fmt::{self, Display},
    io::{self, BufWriter, Write},
    mem,
    os::unix::ffi::OsStrExt,
    result::Result,
    str,
};
use types;

fn list_vars(shell: &Shell) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    // Write all the string variables to the buffer.
    let _ = buffer.write(b"# String Variables\n")?;
    for (key, val) in shell.variables.string_vars() {
        writeln!(buffer, "{} = {}", key, val)?;
    }

    // Then immediately follow that with a list of array variables.
    let _ = buffer.write(b"\n# Array Variables\n")?;
    for (key, val) in shell.variables.arrays() {
        write!(buffer, "{} = [ ", key)?;
        let mut vars = val.iter();
        if let Some(ref var) = vars.next() {
            write!(buffer, "'{}' ", var)?;
            vars.map(|var| write!(buffer, ", '{}' ", var)).collect::<Result<Vec<_>, _>>()?;
        }
        writeln!(buffer, "]")?;
    }
    Ok(())
}

fn arithmetic_op(operator: Operator, value: f64) -> Result<Box<dyn Fn(f64) -> f64>, String> {
    match operator {
        Operator::Add => Ok(Box::new(move |src| src + value)),
        Operator::Divide => Ok(Box::new(move |src| src / value)),
        Operator::Subtract => Ok(Box::new(move |src| src - value)),
        Operator::Multiply => Ok(Box::new(move |src| src * value)),
        _ => Err("operator does not work on arrays".to_string()),
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
            ExportAction::LocalExport(ref key) => match self.get::<types::Str>(key) {
                Some(var) => {
                    env::set_var(key, &*var);
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
                    let _ = writeln!(stdout, "{} = \"{}\"", key, val);
                }
                return SUCCESS;
            }
        };

        for action in actions {
            let err = action.map_err(|e| e.to_string()).and_then(|act| match act {
                Action::UpdateArray(key, Operator::Equal, expression) => {
                    value_check(self, &expression, &key.kind)
                        .map_err(|e| e.to_string())
                        .and_then(|val| match val {
                            VariableType::Array(values) => {
                                env::set_var(key.name, values.join(" "));
                                Ok(())
                            }
                            _ => Err(format!("export of type '{}' is not supported", key.kind)),
                        })
                        .map_err(|why| format!("{}: {}", key.name, why))
                }
                Action::UpdateArray(..) => Err("arithmetic operators on array expressions aren't \
                                                supported yet."
                    .to_string()),
                Action::UpdateString(key, operator, expression) => {
                    value_check(self, &expression, &key.kind)
                        .map_err(|why| format!("{}: {}", key.name, why))
                        .and_then(|val| {
                            if let VariableType::Str(value) = &val {
                                let key_name: &str = &key.name;
                                let lhs = self
                                    .variables
                                    .get::<types::Str>(key_name)
                                    .unwrap_or_else(|| "0".into());

                                math(&lhs, &key.kind, operator, &value, |value| {
                                    let str_value = unsafe { str::from_utf8_unchecked(value) };
                                    if key_name == "PATH" && str_value.find('~').is_some() {
                                        let final_value = str_value.replace(
                                            "~",
                                            env::var("HOME")
                                                .as_ref()
                                                .map(|s| s.as_str())
                                                .unwrap_or("~"),
                                        );
                                        env::set_var(
                                            key_name,
                                            &OsStr::from_bytes(final_value.as_bytes()),
                                        )
                                    } else {
                                        env::set_var(key_name, &OsStr::from_bytes(value))
                                    }
                                    Ok(())
                                })
                                .map_err(|why| format!("{}", why))
                            } else {
                                Ok(())
                            }
                        })
                }
            });

            if let Err(why) = err {
                eprintln!("ion: assignment error: {}", why);
                return FAILURE;
            }
        }

        SUCCESS
    }

    fn local(&mut self, action: LocalAction) -> i32 {
        let mut collected: HashMap<&str, VariableType> = HashMap::new();
        let (actions_step1, actions_step2) = match action {
            LocalAction::List => {
                let _ = list_vars(&self);
                return SUCCESS;
            }
            LocalAction::Assign(ref keys, op, ref vals) => {
                (AssignmentActions::new(keys, op, vals), AssignmentActions::new(keys, op, vals))
            }
        };
        for action in actions_step1 {
            let err = action.map_err(|e| format!("{}", e)).and_then(|act| match act {
                Action::UpdateArray(key, operator, expression) => {
                    let right_hand_side = value_check(self, &expression, &key.kind);

                    right_hand_side.map_err(|why| format!("{}: {}", key.name, why)).and_then(
                        |val| {
                            if [Operator::Equal, Operator::OptionalEqual].contains(&operator) {
                                // When we changed the HISTORY_IGNORE variable, update the
                                // ignore patterns. This happens first because `set_array`
                                // consumes 'values'
                                if key.name == "HISTORY_IGNORE" {
                                    if let VariableType::Array(values) = &val {
                                        self.update_ignore_patterns(values);
                                    }
                                }
                                collected.insert(key.name, val);
                                return Ok(());
                            }

                            let left_hand_side = self
                                .variables
                                .get_mut(key.name)
                                .ok_or_else(|| "cannot concatenate non-array variable".to_string());
                            if let VariableType::Array(values) = val {
                                left_hand_side.map(|v| {
                                    if let VariableType::Array(ref mut array) = v {
                                        match operator {
                                            Operator::Concatenate => array.extend(values),
                                            Operator::ConcatenateHead => values
                                                .into_iter()
                                                .rev()
                                                .for_each(|value| array.insert(0, value)),
                                            Operator::Filter => {
                                                array.retain(|item| !values.contains(item))
                                            }
                                            _ => {}
                                        }
                                    }
                                })
                            } else {
                                Ok(())
                            }
                        },
                    )
                }
                Action::UpdateString(key, operator, expression) => {
                    if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                        return Err(format!("not allowed to set {}", key.name));
                    }

                    let right_hand_side = value_check(self, &expression, &key.kind);

                    let left_hand_side = self.variables.get_mut(key.name).ok_or_else(|| {
                        format!("{}: cannot concatenate non-array variable", key.name)
                    });
                    right_hand_side.map_err(|why| format!("{}: {}", key.name, why)).and_then(
                        |val| {
                            if [Operator::Equal, Operator::OptionalEqual].contains(&operator) {
                                collected.insert(key.name, val);
                                return Ok(());
                            }

                            left_hand_side.and_then(|v| match val {
                                VariableType::Str(value) => match v {
                                    VariableType::Str(lhs) => {
                                        math(&lhs, &key.kind, operator, &value, |value| {
                                            collected.insert(
                                                key.name,
                                                VariableType::Str(
                                                    unsafe { str::from_utf8_unchecked(value) }
                                                        .into(),
                                                ),
                                            );
                                            Ok(())
                                        })
                                        .map_err(|e| format!("{}", e))
                                    }
                                    VariableType::Array(ref mut array) => match operator {
                                        Operator::Concatenate => {
                                            array.push(value);
                                            Ok(())
                                        }
                                        Operator::ConcatenateHead => {
                                            array.insert(0, value);
                                            Ok(())
                                        }
                                        Operator::Filter => {
                                            array.retain(|item| item != &value);
                                            Ok(())
                                        }
                                        _ => value
                                            .parse::<f64>()
                                            .map_err(|_| format!("`{}` is not a float", value))
                                            .and_then(|value| arithmetic_op(operator, value))
                                            .and_then(|action| {
                                                array
                                                    .iter_mut()
                                                    .map(|element| {
                                                        element
                                                            .parse::<f64>()
                                                            .map_err(|_| {
                                                                format!(
                                                                    "`{}` is not a float",
                                                                    element
                                                                )
                                                            })
                                                            .map(|n| action(n))
                                                            .map(|result| {
                                                                *element = result.to_string().into()
                                                            })
                                                    })
                                                    .find(|e| e.is_err())
                                                    .unwrap_or(Ok(()))
                                            }),
                                    },
                                    _ => Err("type does not support this operator".to_string()),
                                },
                                _ => unreachable!(),
                            })
                        },
                    )
                }
            });

            if let Err(why) = err {
                eprintln!("ion: assignment error: {}", why);
                return FAILURE;
            }
        }

        for action in actions_step2 {
            match action.unwrap() {
                Action::UpdateArray(key, operator, ..) => {
                    if operator == Operator::OptionalEqual
                        && self.variables.get_ref(key.name).is_some()
                    {
                        continue;
                    }

                    let err = collected
                        .remove(key.name)
                        .map(|var| match (&var, &key.kind) {
                            (VariableType::HashMap(_), Primitive::Indexed(..)) => {
                                Err("cannot insert hmap into index".to_string())
                            }
                            (VariableType::BTreeMap(_), Primitive::Indexed(..)) => {
                                Err("cannot insert bmap into index".to_string())
                            }
                            (VariableType::Array(_), Primitive::Indexed(..)) => {
                                Err("multi-dimensional arrays are not yet supported".to_string())
                            }
                            (VariableType::HashMap(_), Primitive::HashMap(_))
                            | (VariableType::BTreeMap(_), Primitive::BTreeMap(_))
                            | (VariableType::Array(_), _) => {
                                self.variables.set(key.name, var);
                                Ok(())
                            }
                            (
                                VariableType::Str(_),
                                Primitive::Indexed(ref index_value, ref index_kind),
                            ) => value_check(self, index_value, index_kind)
                                .map_err(|why| format!("assignment error: {}: {}", key.name, why))
                                .and_then(|val| match val {
                                    VariableType::Str(ref index) => {
                                        match self.variables.get_mut(key.name) {
                                            Some(VariableType::HashMap(hmap)) => {
                                                hmap.insert(index.clone(), var);
                                                Ok(())
                                            }
                                            Some(VariableType::BTreeMap(bmap)) => {
                                                bmap.insert(index.clone(), var);
                                                Ok(())
                                            }
                                            Some(VariableType::Array(array)) => index
                                                .parse::<usize>()
                                                .map_err(|_| {
                                                    format!(
                                                        "index variable does not contain a \
                                                         numeric value: {}",
                                                        index
                                                    )
                                                })
                                                .map(|index_num| {
                                                    if let (Some(val), VariableType::Str(value)) =
                                                        (array.get_mut(index_num), var)
                                                    {
                                                        *val = value;
                                                    }
                                                }),
                                            _ => Ok(()),
                                        }
                                    }
                                    VariableType::Array(_) => {
                                        Err("index variable cannot be an array".to_string())
                                    }
                                    VariableType::HashMap(_) => {
                                        Err("index variable cannot be a hmap".to_string())
                                    }
                                    VariableType::BTreeMap(_) => {
                                        Err("index variable cannot be a bmap".to_string())
                                    }
                                    _ => Ok(()),
                                }),
                            _ => Ok(()),
                        })
                        .unwrap_or(Ok(()));

                    if let Err(why) = err {
                        eprintln!("ion: {}", why);
                        return FAILURE;
                    }
                }
                Action::UpdateString(key, operator, ..) => {
                    if operator == Operator::OptionalEqual
                        && self.variables.get_ref(key.name).is_some()
                    {
                        continue;
                    }
                    match collected.remove(key.name) {
                        Some(var @ VariableType::Str(_)) | Some(var @ VariableType::Array(_)) => {
                            self.variables.set(key.name, var);
                        }
                        _ => (),
                    }
                }
            }
        }

        SUCCESS
    }
}

#[derive(Debug)]
enum MathError {
    RHS,
    LHS,
    Unsupported,
    CalculationError,
}

impl Display for MathError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MathError::RHS => write!(fmt, "right hand side has invalid type"),
            MathError::LHS => write!(fmt, "left hand side has invalid type"),
            MathError::Unsupported => write!(fmt, "type does not support operation"),
            MathError::CalculationError => write!(fmt, "cannot calculate given operation"),
        }
    }
}

fn parse_f64<F: Fn(f64, f64) -> f64>(lhs: &str, rhs: &str, operation: F) -> Result<f64, MathError> {
    lhs.parse::<f64>().map_err(|_| MathError::LHS).and_then(|lhs| {
        rhs.parse::<f64>().map_err(|_| MathError::RHS).map(|rhs| operation(lhs, rhs))
    })
}

fn parse_i64<F: Fn(i64, i64) -> Option<i64>>(
    lhs: &str,
    rhs: &str,
    operation: F,
) -> Result<i64, MathError> {
    lhs.parse::<i64>().map_err(|_| MathError::LHS).and_then(|lhs| {
        rhs.parse::<i64>()
            .map_err(|_| MathError::RHS)
            .and_then(|rhs| operation(lhs, rhs).ok_or(MathError::CalculationError))
    })
}

fn write_integer<F: FnMut(&[u8]) -> Result<(), MathError>>(
    integer: i64,
    mut func: F,
) -> Result<(), MathError> {
    let mut buffer: [u8; 20] = unsafe { mem::uninitialized() };
    let capacity = itoa::write(&mut buffer[..], integer).unwrap();
    func(&buffer[..capacity])
}

fn math<'a, F: FnMut(&[u8]) -> Result<(), MathError>>(
    lhs: &str,
    key: &Primitive,
    operator: Operator,
    value: &'a str,
    mut writefn: F,
) -> Result<(), MathError> {
    match operator {
        Operator::Add => match key {
            Primitive::Any | Primitive::Float => {
                writefn(parse_f64(lhs, value, |lhs, rhs| lhs + rhs)?.to_string().as_bytes())
            }
            Primitive::Integer => {
                write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs + rhs))?, writefn)
            }
            _ => Err(MathError::Unsupported),
        },
        Operator::Divide => match key {
            Primitive::Any | Primitive::Float | Primitive::Integer => {
                writefn(parse_f64(lhs, value, |lhs, rhs| lhs / rhs)?.to_string().as_bytes())
            }
            _ => Err(MathError::Unsupported),
        },
        Operator::IntegerDivide => match key {
            Primitive::Any | Primitive::Float => write_integer(
                parse_i64(lhs, value, |lhs, rhs| {
                    // We want to make sure we don't divide by zero, so instead, we give them a None
                    // as a result to signify that we were unable to calculate the result.
                    if rhs == 0 {
                        None
                    } else {
                        Some(lhs / rhs)
                    }
                })?,
                writefn,
            ),
            _ => Err(MathError::Unsupported),
        },
        Operator::Subtract => match key {
            Primitive::Any | Primitive::Float => {
                writefn(parse_f64(lhs, value, |lhs, rhs| lhs - rhs)?.to_string().as_bytes())
            }
            Primitive::Integer => {
                write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs - rhs))?, writefn)
            }
            _ => Err(MathError::Unsupported),
        },
        Operator::Multiply => match key {
            Primitive::Any | Primitive::Float => {
                writefn(parse_f64(lhs, value, |lhs, rhs| lhs * rhs)?.to_string().as_bytes())
            }
            Primitive::Integer => {
                write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs * rhs))?, writefn)
            }
            _ => Err(MathError::Unsupported),
        },
        Operator::Exponent => match key {
            Primitive::Any | Primitive::Float => {
                writefn(parse_f64(lhs, value, |lhs, rhs| lhs.powf(rhs))?.to_string().as_bytes())
            }
            Primitive::Integer => {
                write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs.pow(rhs as u32)))?, writefn)
            }
            _ => Err(MathError::Unsupported),
        },
        Operator::Equal => writefn(value.as_bytes()),
        _ => Err(MathError::Unsupported),
    }
}
