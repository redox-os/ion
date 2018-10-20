use super::{
    flow_control::{ExportAction, LocalAction},
    status::*,
    Shell,
};
use itoa;
use lexers::assignments::{Operator, Primitive};
use parser::assignments::*;
use shell::{history::ShellHistory, variables::VariableType};
use small;
use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fmt::{self, Display},
    io::{self, BufWriter, Write},
    mem,
    os::unix::ffi::OsStrExt,
    str,
};
use types;

fn list_vars(shell: &Shell) {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    // Small function for formatting and append an array entry to a string buffer.
    fn print_array<W: Write>(buffer: &mut W, key: &str, array: &[small::String]) {
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
    for (key, val) in shell.variables.string_vars() {
        let _ = buffer.write([key, " = ", val.as_str(), "\n"].concat().as_bytes());
    }

    // Then immediately follow that with a list of array variables.
    let _ = buffer.write(b"\n# Array Variables\n");
    for (key, val) in shell.variables.arrays() {
        print_array(&mut buffer, &key, &**val)
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
                    let _ = writeln!(stdout, "{} =\"{}\"", key, val);
                }
                return SUCCESS;
            }
        };

        for action in actions {
            match action {
                Ok(Action::UpdateArray(key, Operator::Equal, expression)) => {
                    match value_check(self, &expression, &key.kind) {
                        Ok(VariableType::Array(values)) => env::set_var(key.name, values.join(" ")),
                        Err(why) => {
                            eprintln!("ion: assignment error: {}: {}", key.name, why);
                            return FAILURE;
                        }
                        _ => {
                            eprintln!(
                                "ion: assignment error: export of type '{}' is not supported",
                                key.kind
                            );
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
                    match value_check(self, &expression, &key.kind) {
                        Ok(VariableType::Str(value)) => {
                            let key_name: &str = &key.name;
                            let lhs: types::Str = self
                                .variables
                                .get::<types::Str>(key_name)
                                .unwrap_or_else(|| "0".into());

                            let result = math(&lhs, &key.kind, operator, &value, |value| {
                                let str_value = unsafe {str::from_utf8_unchecked(value)};
                                if key_name == "PATH" && str_value.find('~').is_some() {
                                    let final_value = str_value.replace("~", env::var("HOME").as_ref().map(|s| s.as_str()).unwrap_or("~"));
                                    env::set_var(key_name, &OsStr::from_bytes(final_value.as_bytes()))
                                } else {
                                    env::set_var(key_name, &OsStr::from_bytes(value))
                                }
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
        let mut collected: HashMap<&str, VariableType> = HashMap::new();
        let (actions_step1, actions_step2) = match action {
            LocalAction::List => {
                list_vars(&self);
                return SUCCESS;
            }
            LocalAction::Assign(ref keys, op, ref vals) => (
                AssignmentActions::new(keys, op, vals),
                AssignmentActions::new(keys, op, vals),
            ),
        };
        for action in actions_step1 {
            match action {
                Ok(Action::UpdateArray(key, operator, expression)) => {
                    match operator {
                        Operator::Equal | Operator::OptionalEqual => {
                            match value_check(self, &expression, &key.kind) {
                                Ok(VariableType::Array(values)) => {
                                    // When we changed the HISTORY_IGNORE variable, update the
                                    // ignore patterns. This happens first because `set_array`
                                    // consumes 'values'
                                    if key.name == "HISTORY_IGNORE" {
                                        self.update_ignore_patterns(&values);
                                    }
                                    collected.insert(key.name, VariableType::Array(values));
                                }
                                Ok(VariableType::Str(value)) => {
                                    collected.insert(key.name, VariableType::Str(value));
                                }
                                Ok(VariableType::HashMap(hmap)) => {
                                    collected.insert(key.name, VariableType::HashMap(hmap));
                                }
                                Ok(VariableType::BTreeMap(bmap)) => {
                                    collected.insert(key.name, VariableType::BTreeMap(bmap));
                                }
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}: {}", key.name, why);
                                    return FAILURE;
                                }
                                _ => (),
                            }
                        }
                        Operator::Concatenate => match value_check(self, &expression, &key.kind) {
                            Ok(VariableType::Array(values)) => {
                                match self.variables.get_mut(key.name) {
                                    Some(VariableType::Array(ref mut array)) => {
                                        array.extend(values);
                                    }
                                    None => {
                                        eprintln!(
                                            "ion: assignment error: {}: cannot concatenate \
                                             non-array variable",
                                            key.name
                                        );
                                        return FAILURE;
                                    }
                                    _ => (),
                                }
                            }
                            Err(why) => {
                                eprintln!("ion: assignment error: {}: {}", key.name, why);
                                return FAILURE;
                            }
                            _ => (),
                        },
                        Operator::ConcatenateHead => {
                            match value_check(self, &expression, &key.kind) {
                                Ok(VariableType::Array(values)) => {
                                    match self.variables.get_mut(key.name) {
                                        Some(VariableType::Array(ref mut array)) => {
                                            for (index, value) in values.into_iter().enumerate() {
                                                array.insert(index, value);
                                            }
                                        }
                                        None => {
                                            eprintln!(
                                                "ion: assignment error: {}: cannot head \
                                                 concatenate non-array variable",
                                                key.name
                                            );
                                            return FAILURE;
                                        }
                                        _ => (),
                                    }
                                }
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}: {}", key.name, why);
                                    return FAILURE;
                                }
                                _ => (),
                            }
                        }
                        Operator::Filter => match value_check(self, &expression, &key.kind) {
                            Ok(VariableType::Array(values)) => {
                                match self.variables.get_mut(key.name) {
                                    Some(VariableType::Array(ref mut array)) => {
                                        *array = array
                                            .iter()
                                            .filter(|item| {
                                                values.iter().all(|value| *item != value)
                                            }).cloned()
                                            .collect();
                                    }
                                    None => {
                                        eprintln!(
                                            "ion: assignment error: {}: cannot head concatenate \
                                             non-array variable",
                                            key.name
                                        );
                                        return FAILURE;
                                    }
                                    _ => (),
                                }
                            }
                            Err(why) => {
                                eprintln!("ion: assignment error: {}: {}", key.name, why);
                                return FAILURE;
                            }
                            _ => (),
                        },
                        _ => (),
                    }
                }
                Ok(Action::UpdateString(key, operator, expression)) => {
                    if ["HOME", "HOST", "PWD", "MWD", "SWD", "?"].contains(&key.name) {
                        eprintln!("ion: not allowed to set {}", key.name);
                        return FAILURE;
                    }

                    match value_check(self, &expression, &key.kind) {
                        Ok(VariableType::Str(value)) => {
                            match operator {
                                Operator::Equal | Operator::OptionalEqual => {
                                    collected.insert(key.name, VariableType::Str(value));
                                    continue;
                                }
                                Operator::Concatenate => {
                                    match self.variables.get_mut(key.name) {
                                        Some(VariableType::Array(ref mut array)) => {
                                            array.push(value);
                                        }
                                        None => {
                                            eprintln!(
                                                "ion: assignment error: {}: cannot concatenate \
                                                 non-array variable",
                                                key.name
                                            );
                                            return FAILURE;
                                        }
                                        _ => (),
                                    }
                                    continue;
                                }
                                Operator::ConcatenateHead => {
                                    match self.variables.get_mut(key.name) {
                                        Some(VariableType::Array(ref mut array)) => {
                                            array.insert(0, value);
                                        }
                                        None => {
                                            eprintln!(
                                                "ion: assignment error: {}: cannot head \
                                                 concatenate non-array variable",
                                                key.name
                                            );
                                            return FAILURE;
                                        }
                                        _ => (),
                                    }
                                    continue;
                                }
                                Operator::Filter => {
                                    match self.variables.get_mut(key.name) {
                                        Some(VariableType::Array(ref mut array)) => {
                                            *array = array
                                                .iter()
                                                .filter(move |item| **item != value)
                                                .cloned()
                                                .collect();
                                        }
                                        None => {
                                            eprintln!(
                                                "ion: assignment error: {}: cannot head \
                                                 concatenate non-array variable",
                                                key.name
                                            );
                                            return FAILURE;
                                        }
                                        _ => (),
                                    }
                                    continue;
                                }
                                _ => (),
                            }
                            match self.variables.get_mut(key.name) {
                                Some(VariableType::Str(lhs)) => {
                                    let result = math(&lhs, &key.kind, operator, &value, |value| {
                                        collected.insert(
                                            key.name,
                                            VariableType::Str(
                                                unsafe { str::from_utf8_unchecked(value) }.into(),
                                            ),
                                        );
                                    });

                                    if let Err(why) = result {
                                        eprintln!("ion: assignment error: {}", why);
                                        return FAILURE;
                                    }
                                }
                                Some(VariableType::Array(array)) => {
                                    let value = match value.parse::<f64>() {
                                        Ok(n) => n,
                                        Err(_) => {
                                            eprintln!(
                                                "ion: assignment error: value is not a float"
                                            );
                                            return FAILURE;
                                        }
                                    };

                                    let action: Box<
                                        dyn Fn(f64) -> f64,
                                    > = match operator {
                                        Operator::Add => Box::new(|src| src + value),
                                        Operator::Divide => Box::new(|src| src / value),
                                        Operator::Subtract => Box::new(|src| src - value),
                                        Operator::Multiply => Box::new(|src| src * value),
                                        _ => {
                                            eprintln!(
                                                "ion: assignment error: operator does not work on \
                                                 arrays"
                                            );
                                            return FAILURE;
                                        }
                                    };

                                    for element in array {
                                        match element.parse::<f64>() {
                                            Ok(number) => {
                                                *element = action(number).to_string().into()
                                            }
                                            Err(_) => {
                                                eprintln!(
                                                    "ion: assignment error: {} is not a float",
                                                    element
                                                );
                                                return FAILURE;
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    eprintln!(
                                        "ion: assignment error: type does not support this \
                                         operator"
                                    );
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
                Ok(Action::UpdateArray(key, operator, ..)) => {
                    if operator == Operator::OptionalEqual {
                        match self.variables.get_ref(key.name) {
                            Some(_) => {
                                continue;
                            }
                            _ => (),
                        };
                    }
                    match collected.remove(key.name) {
                        hmap @ Some(VariableType::HashMap(_)) => {
                            if let Primitive::HashMap(_) = key.kind {
                                self.variables.set(key.name, hmap.unwrap());
                            } else if let Primitive::Indexed(..) = key.kind {
                                eprintln!("ion: cannot insert hmap into index");
                                return FAILURE;
                            }
                        }
                        bmap @ Some(VariableType::BTreeMap(_)) => {
                            if let Primitive::BTreeMap(_) = key.kind {
                                self.variables.set(key.name, bmap.unwrap());
                            } else if let Primitive::Indexed(..) = key.kind {
                                eprintln!("ion: cannot insert bmap into index");
                                return FAILURE;
                            }
                        }
                        array @ Some(VariableType::Array(_)) => {
                            if let Primitive::Indexed(..) = key.kind {
                                eprintln!("ion: multi-dimensional arrays are not yet supported");
                                return FAILURE;
                            } else {
                                self.variables.set(key.name, array.unwrap());
                            }
                        }
                        Some(VariableType::Str(value)) => {
                            if let Primitive::Indexed(ref index_value, ref index_kind) = key.kind {
                                match value_check(self, index_value, index_kind) {
                                    Ok(VariableType::Str(ref index)) => {
                                        match self.variables.get_mut(key.name) {
                                            Some(VariableType::HashMap(hmap)) => {
                                                hmap.entry(index.clone())
                                                    .or_insert_with(|| VariableType::Str(value));
                                            }
                                            Some(VariableType::BTreeMap(bmap)) => {
                                                bmap.entry(index.clone())
                                                    .or_insert_with(|| VariableType::Str(value));
                                            }
                                            Some(VariableType::Array(array)) => {
                                                let index_num = match index.parse::<usize>() {
                                                    Ok(num) => num,
                                                    Err(_) => {
                                                        eprintln!(
                                                            "ion: index variable does not contain \
                                                             a numeric value: {}",
                                                            index
                                                        );
                                                        return FAILURE;
                                                    }
                                                };
                                                if let Some(val) = array.get_mut(index_num) {
                                                    *val = value;
                                                }
                                            }
                                            _ => (),
                                        }
                                    }
                                    Ok(VariableType::Array(_)) => {
                                        eprintln!("ion: index variable cannot be an array");
                                        return FAILURE;
                                    }
                                    Ok(VariableType::HashMap(_)) => {
                                        eprintln!("ion: index variable cannot be a hmap");
                                        return FAILURE;
                                    }
                                    Ok(VariableType::BTreeMap(_)) => {
                                        eprintln!("ion: index variable cannot be a bmap");
                                        return FAILURE;
                                    }
                                    Err(why) => {
                                        eprintln!("ion: assignment error: {}: {}", key.name, why);
                                        return FAILURE;
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
                Ok(Action::UpdateString(key, operator, ..)) => {
                    if operator == Operator::OptionalEqual {
                        match self.variables.get_ref(key.name) {
                            Some(_) => {
                                continue;
                            }
                            _ => (),
                        };
                    }
                    match collected.remove(key.name) {
                        str_ @ Some(VariableType::Str(_)) => {
                            self.variables.set(key.name, str_.unwrap());
                        }
                        array @ Some(VariableType::Array(_)) => {
                            self.variables.set(key.name, array.unwrap());
                        }
                        _ => (),
                    }
                }
                _ => unreachable!(),
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
    CalculationError
}

impl Display for MathError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MathError::RHS => write!(fmt, "right hand side has invalid type"),
            MathError::LHS => write!(fmt, "left hand side has invalid type"),
            MathError::Unsupported => write!(fmt, "type does not support operation"),
            MathError::CalculationError => write!(fmt, "cannot calculate given operation")
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

fn parse_i64<F: Fn(i64, i64) -> Option<i64>>(lhs: &str, rhs: &str, operation: F) -> Result<i64, MathError> {
    let lhs = match lhs.parse::<i64>() {
        Ok(e) => Ok(e),
        Err(_) => Err(MathError::LHS)
    };
    if let Ok(lhs) = lhs {
        let rhs = match rhs.parse::<i64>() {
            Ok(e) => Ok(e),
            Err(_) => Err(MathError::RHS)
        };
        if let Ok(rs) = rhs {
            let ret = operation(lhs, rs);
            if let Some(n) = ret {
                Ok(n)
            } else {
                Err(MathError::CalculationError)
            }
        } else {
            rhs
        }
    } else {
        lhs
    }
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
            write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs + rhs))?, writefn);
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
            write_integer(parse_i64(lhs, value, |lhs, rhs| {
                // We want to make sure we don't divide by zero, so instead, we give them a None as a result to signify that we were unable to calculate the result.
                if rhs == 0 {
                    None
                } else {
                    Some(lhs / rhs)
                }
            })?, writefn);
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
            write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs - rhs))?, writefn);
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
            write_integer(parse_i64(lhs, value, |lhs, rhs| Some(lhs * rhs))?, writefn);
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
                parse_i64(lhs, value, |lhs, rhs| Some(lhs.pow(rhs as u32)))?,
                writefn,
            );
        } else {
            return Err(MathError::Unsupported);
        },
        Operator::Equal => writefn(value.as_bytes()),
        _ => return Err(MathError::Unsupported),
    };

    Ok(())
}
