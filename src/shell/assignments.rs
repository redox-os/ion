use std::env;
use std::io::{self, Write};

use smallstring::SmallString;
use super::history::ShellHistory;
use super::Shell;
use super::status::*;
use parser::expand_string;
use parser::types::assignments::*;
use parser::types::parse::*;
use smallvec::SmallVec;
use types::{Array, ArrayVariableContext, VariableContext};

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
        let _ = stdout.write(key.as_bytes()).and_then(
            |_| stdout.write_all(b" = [ \""),
        );

        let mut elements = value.iter();

        if let Some(element) = elements.next() {
            let _ = stdout.write_all(element.as_bytes());
        }

        for element in elements {
            let _ = stdout.write_all(b"\" \"").and_then(|_| {
                stdout.write_all(element.as_bytes())
            });
        }

        let _ = stdout.write(b"\" ]\n");
    }
}

/// Represents: A variable store capable of setting local variables or
/// exporting variables to some global environment
pub trait VariableStore {
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
                            let values = expand_string(expression, self, false);
                            let use_original = match array_is_valid(&values, key.kind) {
                                Ok(Some(normalized)) => {
                                    self.variables.set_array(key.name, normalized);
                                    false
                                }
                                Ok(None) => true,
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}", why);
                                    return FAILURE;
                                }
                            };

                            if use_original {
                                // When we changed the HISTORY_IGNORE variable, update the ignore
                                // patterns. This happens first because `set_array` consumes 'values'
                                if key.name == "HISTORY_IGNORE" {
                                    self.update_ignore_patterns(&values);
                                }
                                self.variables.set_array(key.name, values);
                            }
                        }
                        Ok(Action::UpdateArray(..)) => {
                            eprintln!("ion: arithmetic operators on array expressions aren't supported yet.");
                            return FAILURE;
                        }
                        Ok(Action::UpdateString(key, operator, expression)) => {
                            let value = expand_string(expression, self, false).join(" ");
                            let value = match string_is_valid(&value, key.kind) {
                                Ok(value) => value,
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}", why);
                                    return FAILURE;
                                }
                            };

                            if !integer_math(self, key, operator, &value) {
                                return FAILURE;
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
            Ok(assignment_actions) => {
                for action in assignment_actions {
                    match action {
                        Ok(Action::UpdateArray(key, Operator::Equal, expression)) => {
                            let values = expand_string(expression, self, false);
                            let use_original = match array_is_valid(&values, key.kind) {
                                Ok(Some(normalized)) => {
                                    env::set_var(key.name, normalized.join(" "));
                                    false
                                }
                                Ok(None) => true,
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}", why);
                                    return FAILURE;
                                }
                            };

                            if use_original {
                                env::set_var(key.name, values.join(" "));
                            }
                        }
                        Ok(Action::UpdateArray(..)) => {
                            eprintln!("ion: arithmetic operators on array expressions aren't supported yet.");
                            return FAILURE;
                        }
                        Ok(Action::UpdateString(key, operator, expression)) => {
                            let value = expand_string(expression, self, false).join(" ");
                            let value = match string_is_valid(&value, key.kind) {
                                Ok(value) => value,
                                Err(why) => {
                                    eprintln!("ion: assignment error: {}", why);
                                    return FAILURE;
                                }
                            };

                            if !integer_math_export(key, operator, value) {
                                return FAILURE;
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

fn is_boolean(value: &str) -> Result<&str, ()> {
    if ["true", "1", "y"].contains(&value) {
        Ok("true")
    } else if ["false", "0", "n"].contains(&value) {
        Ok("false")
    } else {
        Err(())
    }
}

fn string_is_valid(value: &str, expected: TypePrimitive) -> Result<&str, TypeError> {
    match expected {
        TypePrimitive::Any | TypePrimitive::Str => Ok(value),
        TypePrimitive::Boolean => is_boolean(value).map_err(|_| TypeError::BadValue(expected)),
        TypePrimitive::Integer => {
            if value.parse::<i64>().is_ok() {
                Ok(value)
            } else {
                Err(TypeError::BadValue(expected))
            }
        }
        TypePrimitive::Float => {
            if value.parse::<f64>().is_ok() {
                Ok(value)
            } else {
                Err(TypeError::BadValue(expected))
            }
        }
        _ => unreachable!(),
    }
}

fn array_is_valid(values: &[String], expected: TypePrimitive) -> Result<Option<Array>, TypeError> {
    match expected {
        TypePrimitive::Any | TypePrimitive::AnyArray | TypePrimitive::StrArray => Ok(None),
        TypePrimitive::BooleanArray => {
            let mut output = SmallVec::new();
            for value in values {
                let value = is_boolean(value.as_str())
                    .map_err(|_| TypeError::BadValue(expected))?
                    .into();
                output.push(value);
            }
            Ok(Some(output))
        }
        TypePrimitive::IntegerArray => {
            for value in values {
                if !value.parse::<i64>().is_ok() {
                    return Err(TypeError::BadValue(expected));
                }
            }
            Ok(None)
        }
        TypePrimitive::FloatArray => {
            for value in values {
                if !value.parse::<f64>().is_ok() {
                    return Err(TypeError::BadValue(expected));
                }
            }
            Ok(None)
        }
        _ => unreachable!(),
    }
}

// NOTE: Here there be excessively long functions.

fn integer_math(shell: &mut Shell, key: TypeArg, operator: Operator, value: &str) -> bool {
    match operator {
        Operator::Add => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs + rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs + rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Divide => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs / rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs / rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Subtract => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs - rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs - rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Multiply => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs * rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs * rhs).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Exponent => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs.powf(rhs)).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match shell.variables.get_var_or_empty(key.name).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<u32>() {
                            Ok(rhs) => {
                                let value = (lhs.pow(rhs)).to_string();
                                shell.variables.set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Equal => {
            shell.variables.set_var(key.name, &value);
            true
        }
    }
}

fn integer_math_export(key: TypeArg, operator: Operator, value: &str) -> bool {
    match operator {
        Operator::Add => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs + rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs + rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Divide => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs / rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs / rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Subtract => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs - rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs - rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Multiply => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs * rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<i64>() {
                            Ok(rhs) => {
                                let value = (lhs * rhs).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Exponent => {
            if TypePrimitive::Any == key.kind || TypePrimitive::Float == key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<f64>() {
                    Ok(lhs) => {
                        match value.parse::<f64>() {
                            Ok(rhs) => {
                                let value = (lhs.powf(rhs)).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else if let TypePrimitive::Integer = key.kind {
                match env::var(key.name).unwrap_or("".into()).parse::<i64>() {
                    Ok(lhs) => {
                        match value.parse::<u32>() {
                            Ok(rhs) => {
                                let value = (lhs.pow(rhs)).to_string();
                                env::set_var(key.name, &value);
                                return true;
                            }
                            Err(_) => eprintln!("ion: right hand side has invalid value type"),
                        }
                    }
                    Err(_) => eprintln!("ion: variable has invalid value type"),
                }
                return false;
            } else {
                eprintln!("ion: variable does not support this operation");
                return false;
            }
        }
        Operator::Equal => {
            env::set_var(key.name, &value);
            true
        }
    }
}
