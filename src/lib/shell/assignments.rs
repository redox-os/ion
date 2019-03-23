use super::{
    flow_control::{ExportAction, LocalAction},
    status::*,
    Shell,
};
use crate::{
    lexers::assignments::{Key, Operator, Primitive},
    parser::{assignments::*, statement::parse::is_valid_name},
    shell::{history::ShellHistory, variables::Value},
    types,
};
use std::{
    env,
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
    fn calculate<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<Vec<(Key<'a>, Value)>, String>;
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

    fn calculate<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<Vec<(Key<'a>, Value)>, String> {
        let mut backup: Vec<_> = Vec::new();
        for action in actions.map(|act| act.map_err(|e| e.to_string())) {
            action
                .and_then(|Action(key, operator, expression)| {
                    // sanitize variable names
                    if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                        Err(format!("not allowed to set `{}`", key.name))
                    } else if !is_valid_name(key.name) {
                        Err("invalid variable name\nVariable names may only have A-Z, a-z, 0-9 \
                             and _\nThe first character cannot be a digit"
                            .into())
                    } else {
                        Ok((key, operator, expression))
                    }
                })
                .and_then(|(key, operator, expression)| {
                    value_check(self, &expression, &key.kind)
                        .map_err(|why| format!("{}: {}", key.name, why))
                        .map(|rhs| (key, operator, rhs))
                })
                .and_then(|(key, operator, rhs)| {
                    if operator == Operator::OptionalEqual
                        && self.variables.get_ref(key.name).is_some()
                    {
                        Ok(())
                    } else if [Operator::Equal, Operator::OptionalEqual].contains(&operator) {
                        // When we changed the HISTORY_IGNORE variable, update the
                        // ignore patterns. This happens first because `set_array`
                        // consumes 'values'
                        if key.name == "HISTORY_IGNORE" {
                            if let Value::Array(array) = &rhs {
                                self.update_ignore_patterns(array);
                            }
                        }

                        match (&rhs, &key.kind) {
                            (Value::HashMap(_), Primitive::Indexed(..)) => {
                                Err("cannot insert hmap into index".to_string())
                            }
                            (Value::BTreeMap(_), Primitive::Indexed(..)) => {
                                Err("cannot insert bmap into index".to_string())
                            }
                            (Value::Array(_), Primitive::Indexed(..)) => {
                                Err("multi-dimensional arrays are not yet supported".to_string())
                            }
                            _ => {
                                backup.push((key, rhs));
                                Ok(())
                            }
                        }
                    } else {
                        self.overwrite(key, operator, rhs)
                    }
                })?;
        }
        Ok(backup)
    }

    fn local(&mut self, action: LocalAction) -> i32 {
        match action {
            LocalAction::List => {
                let _ = list_vars(&self);
                SUCCESS
            }
            LocalAction::Assign(ref keys, op, ref vals) => {
                let actions = AssignmentActions::new(keys, op, vals);
                if let Err(why) = self.calculate(actions).and_then(|apply| {
                    for (key, value) in apply {
                        self.assign(key, value)?
                    }
                    Ok(())
                }) {
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
pub enum MathError {
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

pub fn parse<T: str::FromStr, F: Fn(T) -> Option<f64>>(
    lhs: &str,
    operation: F,
) -> Result<String, MathError> {
    lhs.parse::<T>()
        .map_err(|_| MathError::LHS)
        .and_then(|lhs| operation(lhs).ok_or(MathError::CalculationError))
        .map(|x| x.to_string())
}

pub fn math<'a>(
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
