use super::{
    flow_control::{ExportAction, LocalAction},
    status::*,
    Shell,
};
use crate::{
    lexers::assignments::{Key, Operator, Primitive},
    parser::assignments::*,
    shell::{history::ShellHistory, variables::Value},
    types,
};
use hashbrown::HashMap;

use std::{
    env,
    ffi::OsString,
    fmt::{self, Display},
    io::{self, BufWriter, Write},
    result::Result,
    str,
};

fn list_vars(shell: &Shell) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    // Write all the string variables to the buffer.
    buffer.write_all(b"# String Variables\n")?;
    for (key, val) in shell.variables.string_vars() {
        writeln!(buffer, "{} = {}", key, val)?;
    }

    // Then immediately follow that with a list of array variables.
    buffer.write_all(b"\n# Array Variables\n")?;
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

/// Represents: A variable store capable of setting local variables or
/// exporting variables to some global environment
pub(crate) trait VariableStore {
    /// Set a local variable given a binding
    fn local(&mut self, action: LocalAction) -> i32;
    /// Export a variable to the process environment given a binding
    fn export(&mut self, action: ExportAction) -> i32;
    /// Collect all updates to perform on variables for a given assignement action
    fn create_patch<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<HashMap<Key<'a>, Value>, String>;
    fn patch(&mut self, patch: HashMap<Key, Value>) -> Result<(), String>;
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
            let err = action.map_err(|e| e.to_string()).and_then(|act| {
                let Action(key, operator, expression) = act;
                value_check(self, &expression, &key.kind)
                    .map_err(|e| format!("{}: {}", key.name, e))
                    // TODO: handle operators here in the same way as local
                    .and_then(|rhs| match &rhs {
                        Value::Array(values) if operator == Operator::Equal => {
                            env::set_var(key.name, values.join(" "));
                            Ok(())
                        }
                        Value::Array(_) => Err("arithmetic operators on array expressions aren't \
                                                supported yet."
                            .to_string()),
                        Value::Str(rhs) => {
                            env::set_var(&key.name, rhs.as_str());
                            Ok(())
                        }
                        _ => Err(format!(
                            "{}: export of type '{}' is not supported",
                            key.name, key.kind
                        )),
                    })
            });

            if let Err(why) = err {
                eprintln!("ion: assignment error: {}", why);
                return FAILURE;
            }
        }

        SUCCESS
    }

    fn create_patch<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<HashMap<Key<'a>, Value>, String> {
        let mut patch = HashMap::new();
        actions
            .map(|act| act.map_err(|e| e.to_string()))
            .map(|action| {
                action
                    .and_then(|act| {
                        // sanitize variable names
                        if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&act.0.name) {
                            Err(format!("not allowed to set `{}`", act.0.name))
                        } else {
                            Ok(act)
                        }
                    })
                    .and_then(|Action(key, operator, expression)| {
                        value_check(self, &expression, &key.kind)
                            .map_err(|why| format!("{}: {}", key.name, why))
                            .map(|rhs| (rhs, key, operator))
                    })
                    .and_then(|(rhs, key, operator)| {
                        if operator == Operator::OptionalEqual
                            && self.variables.get_ref(key.name).is_some()
                        {
                            return Ok(());
                        }
                        if [Operator::Equal, Operator::OptionalEqual].contains(&operator) {
                            // When we changed the HISTORY_IGNORE variable, update the
                            // ignore patterns. This happens first because `set_array`
                            // consumes 'values'
                            if key.name == "HISTORY_IGNORE" {
                                if let Value::Array(array) = &rhs {
                                    self.update_ignore_patterns(array);
                                }
                            }

                            return match (&rhs, &key.kind) {
                                (Value::HashMap(_), Primitive::Indexed(..)) => {
                                    Err("cannot insert hmap into index".to_string())
                                }
                                (Value::BTreeMap(_), Primitive::Indexed(..)) => {
                                    Err("cannot insert bmap into index".to_string())
                                }
                                (Value::Array(_), Primitive::Indexed(..)) => {
                                    Err("multi-dimensional arrays are not yet supported"
                                        .to_string())
                                }
                                _ => {
                                    patch.insert(key, rhs);
                                    Ok(())
                                }
                            };
                        }

                        self.variables
                            .get_mut(key.name)
                            .ok_or_else(|| {
                                format!("cannot update non existing variable `{}`", key.name)
                            })
                            .and_then(|lhs| match rhs {
                                Value::Str(rhs) => match lhs {
                                    Value::Str(lhs) => math(&key.kind, operator, &rhs)
                                        .and_then(|action| parse(&lhs, &*action))
                                        .map(|value| {
                                            patch.insert(key, Value::Str(value.into()));
                                        })
                                        .map_err(|why| why.to_string()),
                                    Value::Array(ref mut array) => match operator {
                                        Operator::Concatenate => {
                                            array.push(rhs);
                                            Ok(())
                                        }
                                        Operator::ConcatenateHead => {
                                            array.insert(0, rhs);
                                            Ok(())
                                        }
                                        Operator::Filter => {
                                            array.retain(|item| item != &rhs);
                                            Ok(())
                                        }
                                        _ => math(&Primitive::Float, operator, &rhs)
                                            .and_then(|action| {
                                                array
                                                    .iter_mut()
                                                    .map(|el| {
                                                        parse(el, &*action)
                                                            .map(|result| *el = result.into())
                                                    })
                                                    .find(|e| e.is_err())
                                                    .unwrap_or(Ok(()))
                                            })
                                            .map_err(|why| why.to_string()),
                                    },
                                    _ => Err("type does not support this operator".to_string()),
                                },
                                Value::Array(values) => {
                                    if let Value::Array(ref mut array) = lhs {
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
                                    Ok(())
                                }
                                _ => unreachable!(),
                            })
                    })
            })
            .find(|e| e.is_err())
            .unwrap_or_else(|| Ok(()))
            .map(|_| patch)
    }

    fn patch(&mut self, patch: HashMap<Key, Value>) -> Result<(), String> {
        patch
            .into_iter()
            .map(|(key, val)| match (&val, &key.kind) {
                (Value::Str(_), Primitive::Indexed(ref index_name, ref index_kind)) => {
                    value_check(self, index_name, index_kind)
                        .map_err(|why| format!("assignment error: {}: {}", key.name, why))
                        .and_then(|index| match index {
                            Value::Array(_) => Err("index variable cannot be an array".to_string()),
                            Value::HashMap(_) => Err("index variable cannot be a hmap".to_string()),
                            Value::BTreeMap(_) => {
                                Err("index variable cannot be a bmap".to_string())
                            }
                            Value::Str(ref index) => self
                                .variables
                                .get_mut(key.name)
                                .map(|lhs| match lhs {
                                    Value::HashMap(hmap) => {
                                        hmap.insert(index.clone(), val);
                                        Ok(())
                                    }
                                    Value::BTreeMap(bmap) => {
                                        bmap.insert(index.clone(), val);
                                        Ok(())
                                    }
                                    Value::Array(array) => index
                                        .parse::<usize>()
                                        .map_err(|_| {
                                            format!(
                                                "index variable is not a numeric value: `{}`",
                                                index
                                            )
                                        })
                                        .map(|index_num| {
                                            if let (Some(var), Value::Str(value)) =
                                                (array.get_mut(index_num), val)
                                            {
                                                *var = value;
                                            }
                                        }),
                                    _ => Ok(()),
                                })
                                .unwrap_or(Ok(())),
                            _ => Ok(()),
                        })
                }
                (Value::Str(_), _)
                | (Value::HashMap(_), Primitive::HashMap(_))
                | (Value::BTreeMap(_), Primitive::BTreeMap(_))
                | (Value::Array(_), _) => {
                    self.variables.set(key.name, val);
                    Ok(())
                }
                _ => Ok(()),
            })
            .find(|e| e.is_err())
            .unwrap_or_else(|| Ok(()))
    }

    fn local(&mut self, action: LocalAction) -> i32 {
        match action {
            LocalAction::List => {
                let _ = list_vars(&self);
                SUCCESS
            }
            LocalAction::Assign(ref keys, op, ref vals) => {
                let actions = AssignmentActions::new(keys, op, vals);
                if let Err(why) = self.create_patch(actions).and_then(|patch| self.patch(patch)) {
                    eprintln!("ion: assignment error: {}", why);
                    FAILURE
                } else {
                    SUCCESS
                }
            }
        }
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

fn parse<T: str::FromStr, F: Fn(T) -> Option<f64>>(
    lhs: &str,
    operation: F,
) -> Result<String, MathError> {
    lhs.parse::<T>()
        .map_err(|_| MathError::LHS)
        .and_then(|lhs| operation(lhs).ok_or(MathError::CalculationError))
        .map(|x| x.to_string())
}

fn math<'a>(
    key: &Primitive,
    operator: Operator,
    value: &'a str,
) -> Result<Box<Fn(f64) -> Option<f64>>, MathError> {
    // TODO: We should not assume or parse f64 by default.
    //       Decimal precision should be preferred.
    //       128-bit decimal precision should be supported again.
    value.parse::<f64>().map_err(|_| MathError::RHS).and_then(
        |rhs| -> Result<Box<Fn(f64) -> Option<f64>>, MathError> {
            match key {
                Primitive::Str | Primitive::Float | Primitive::Integer => match operator {
                    Operator::Add => Ok(Box::new(move |lhs| Some(lhs + rhs))),
                    Operator::Divide => Ok(Box::new(move |lhs| Some(lhs / rhs))),
                    Operator::IntegerDivide => Ok(Box::new(move |lhs| {
                        (lhs as i64).checked_div(rhs as i64).map(|x| x as f64)
                    })),
                    Operator::Subtract => Ok(Box::new(move |lhs| Some(lhs - rhs))),
                    Operator::Multiply => Ok(Box::new(move |lhs| Some(lhs * rhs))),
                    Operator::Exponent => Ok(Box::new(move |lhs| Some(lhs.powf(rhs)))),
                    _ => Err(MathError::Unsupported),
                },
                _ => Err(MathError::Unsupported),
            }
        },
    )
}
