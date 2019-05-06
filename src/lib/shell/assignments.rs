use super::{
    flow_control::{ExportAction, LocalAction},
    status::*,
    Shell,
};
use crate::{
    lexers::assignments::{Key, Operator, Primitive},
    parser::{assignments::*, statement::parse::is_valid_name},
    shell::variables::{EuclDiv, Modifications, OpError, Pow, Value},
    types,
};
use std::{
    env,
    io::{self, BufWriter, Write},
    result::Result,
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
pub(crate) trait VariableStore<'b> {
    /// Set a local variable given a binding
    fn local(&mut self, action: &LocalAction) -> i32;
    /// Export a variable to the process environment given a binding
    fn export(&mut self, action: &ExportAction) -> i32;
    /// Collect all updates to perform on variables for a given assignement action
    fn calculate<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<Vec<(Key<'a>, Value<'b>)>, String>;
}

impl<'b> VariableStore<'b> for Shell<'b> {
    fn export(&mut self, action: &ExportAction) -> i32 {
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
                                Value::Array(values) if operator == Operator::Equal => {
                                    env::set_var(key.name, values.join(" "));
                                    Ok(())
                                }
                                Value::Array(_) => Err("arithmetic operators on array \
                                                        expressions aren't supported yet."
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
            ExportAction::LocalExport(ref key) => match self.get::<types::Str>(key) {
                Some(var) => {
                    env::set_var(key, &*var);
                    SUCCESS
                }
                None => {
                    eprintln!("ion: cannot export {} because it does not exist.", key);
                    FAILURE
                }
            },
            ExportAction::List => {
                let stdout = io::stdout();
                let mut stdout = stdout.lock();
                for (key, val) in env::vars() {
                    let _ = writeln!(stdout, "{} = \"{}\"", key, val);
                }
                SUCCESS
            }
        }
    }

    fn calculate<'a>(
        &mut self,
        actions: AssignmentActions<'a>,
    ) -> Result<Vec<(Key<'a>, Value<'b>)>, String> {
        let mut backup: Vec<_> = Vec::with_capacity(4);
        for action in actions {
            let Action(key, operator, expression) = action.map_err(|e| e.to_string())?;

            // sanitize variable names
            if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                return Err(format!("not allowed to set `{}`", key.name));
            }

            if !is_valid_name(key.name) {
                return Err("invalid variable name\nVariable names may only be (unicode) \
                            alphanumeric or `_`\nThe first character must be alphabetic"
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

    fn local(&mut self, action: &LocalAction) -> i32 {
        match action {
            LocalAction::List => {
                let _ = list_vars(&self);
                SUCCESS
            }
            LocalAction::Assign(ref keys, op, ref vals) => {
                let actions = AssignmentActions::new(keys, *op, vals);
                if let Err(why) = self.calculate(actions).and_then(|apply| {
                    for (key, value) in apply {
                        self.assign(&key, value)?
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

// This should logically be a method over iterator, but Value is only accessible in the main repo
// TODO: too much allocations occur over here. We need to expand variables before they get
// parsed
fn apply<'b>(op: Operator, lhs: &Value<'b>, rhs: Value) -> Result<Value<'b>, OpError> {
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
        Operator::Filter => match (lhs.clone(), rhs) {
            (Value::Array(mut array), Value::Str(rhs)) => {
                array.retain(|item| item != &rhs);
                Ok(Value::Array(array))
            }
            (Value::Array(mut array), Value::Array(values)) => {
                array.retain(|item| !values.contains(item));
                Ok(Value::Array(array))
            }
            _ => Err(OpError::TypeError),
        },
        _ => unreachable!(),
    }
}
