use super::{
    flow_control::{ExportAction, LocalAction},
    Shell,
};
use crate::{
    assignments::*,
    builtins::Status,
    parser::{
        is_valid_name,
        lexers::assignments::{Key, Operator, Primitive},
    },
    shell::{flow_control::Function, Value},
};
use std::{
    env,
    io::{self, BufWriter, Write},
    result::Result,
};
use types_rs::{EuclDiv, Modifications, OpError, Pow};

fn list_vars(shell: &Shell<'_>) -> Result<(), io::Error> {
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
impl<'b> Shell<'b> {
    /// Export a variable to the process environment given a binding
    pub fn export(&mut self, action: &ExportAction) -> Status {
        match action {
            ExportAction::Assign(ref keys, op, ref vals) => {
                let actions = AssignmentActions::new(keys, *op, vals);

                for action in actions {
                    let err = action.map_err(|e| e.to_string()).and_then(|act| {
                        let Action(key, operator, expression) = act;
                        value_check(self, &expression, &key.kind)
                            .map_err(|e| format!("{}: {}", key.name, e))
                            // TODO: handle operators here in the same way as local
                            .and_then(|rhs| match &rhs {
                                Value::Array(_) if operator == Operator::Equal => {
                                    env::set_var(key.name, format!("{}", rhs));
                                    Ok(())
                                }
                                Value::Array(_) => Err("arithmetic operators on array \
                                                        expressions aren't supported yet."
                                    .to_string()),
                                Value::Str(_) => {
                                    env::set_var(&key.name, &format!("{}", rhs));
                                    Ok(())
                                }
                                _ => Err(format!(
                                    "{}: export of type '{}' is not supported",
                                    key.name, key.kind
                                )),
                            })
                    });

                    if let Err(why) = err {
                        return Status::error(format!("ion: assignment error: {}", why));
                    }
                }

                Status::SUCCESS
            }
            ExportAction::LocalExport(ref key) => match self.variables.get_str(key) {
                Ok(var) => {
                    env::set_var(key, &*var);
                    Status::SUCCESS
                }
                Err(_) => {
                    Status::error(format!("ion: cannot export {} because it does not exist.", key))
                }
            },
            ExportAction::List => {
                let stdout = io::stdout();
                let mut stdout = stdout.lock();
                for (key, val) in env::vars() {
                    let _ = writeln!(stdout, "{} = \"{}\"", key, val);
                }
                Status::SUCCESS
            }
        }
    }

    /// Collect all updates to perform on variables for a given assignment action
    pub(crate) fn calculate<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<Vec<(Key<'a>, Value<Function<'b>>)>, String> {
        let mut backup: Vec<_> = Vec::with_capacity(4);
        for action in actions {
            let Action(key, operator, expression) = action.map_err(|e| e.to_string())?;

            // sanitize variable names
            if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                return Err(format!("not allowed to set `{}`", key.name));
            }

            if !is_valid_name(key.name) {
                return Err("invalid variable name\nVariable names may only be (unicode) \
                            alphanumeric or `_`\nThe first character must be alphabetic or `_`"
                    .to_string());
            }

            if operator == Operator::OptionalEqual && self.variables.get_ref(key.name).is_some() {
                continue;
            }

            let rhs = value_check(self, &expression, &key.kind)
                .map_err(|why| format!("{}: {}", key.name, why))?;

            match (&rhs, &key.kind) {
                (Value::HashMap(_), Primitive::Indexed(..)) => {
                    Err("cannot insert hmap into index".to_string())?
                }
                (Value::BTreeMap(_), Primitive::Indexed(..)) => {
                    Err("cannot insert bmap into index".to_string())?
                }
                (Value::Array(_), Primitive::Indexed(..)) => {
                    Err("multi-dimensional arrays are not yet supported".to_string())?
                }
                _ if [Operator::Equal, Operator::OptionalEqual].contains(&operator) => {
                    backup.push((key, rhs))
                }
                _ => {
                    let lhs = self.variables.get_ref(key.name).ok_or_else(|| {
                        format!("cannot update non existing variable `{}`", key.name)
                    })?;
                    let val = apply(operator, &lhs, rhs).map_err(|_| {
                        format!(
                            "type error: variable `{}` of type `{}` does not support operator",
                            key.name, key.kind
                        )
                    })?;
                    backup.push((key, val));
                }
            }
        }
        Ok(backup)
    }

    /// Set a local variable given a binding
    pub fn local(&mut self, action: &LocalAction) -> Status {
        match action {
            LocalAction::List => {
                let _ = list_vars(&self);
                Status::SUCCESS
            }
            LocalAction::Assign(ref keys, op, ref vals) => {
                let actions = AssignmentActions::new(keys, *op, vals);
                if let Err(why) = self.calculate(actions).and_then(|apply| {
                    for (key, value) in apply {
                        self.assign(&key, value)?
                    }
                    Ok(())
                }) {
                    Status::error(format!("ion: assignment error: {}", why))
                } else {
                    Status::SUCCESS
                }
            }
        }
    }
}

// This should logically be a method over operator, but Value is only accessible in the main repo
// TODO: too much allocations occur over here. We need to expand variables before they get
// parsed
fn apply<'a>(
    op: Operator,
    lhs: &Value<Function<'a>>,
    rhs: Value<Function<'a>>,
) -> Result<Value<Function<'a>>, OpError> {
    match op {
        Operator::Add => lhs + rhs,
        Operator::Divide => lhs / rhs,
        Operator::IntegerDivide => lhs.eucl_div(rhs),
        Operator::Subtract => lhs - rhs,
        Operator::Multiply => lhs * rhs,
        Operator::Exponent => lhs.pow(rhs),
        Operator::Concatenate => {
            let mut lhs = lhs.clone();
            lhs.append(rhs);
            Ok(lhs)
        }
        Operator::ConcatenateHead => {
            let mut lhs = lhs.clone();
            lhs.prepend(rhs);
            Ok(lhs)
        }
        Operator::Filter => match (&lhs, &rhs) {
            (Value::Array(ref array), Value::Str(_)) => {
                // TODO: this should be avoided, but for now values are expanded too late, so we
                // must store copies of arrays to update
                let mut array = array.clone();
                array.retain(|item| item != &rhs);
                Ok(Value::Array(array))
            }
            (Value::Array(ref array), Value::Array(values)) => {
                // TODO: this should be avoided, but for now values are expanded too late, so we
                // must store copies of arrays to update
                let mut array = array.clone();
                array.retain(|item| !values.contains(item));
                Ok(Value::Array(array))
            }
            _ => Err(OpError::TypeError),
        },
        _ => unreachable!(),
    }
}
