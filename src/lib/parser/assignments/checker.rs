use super::super::{expand_string, Expander};
use crate::{
    lexers::assignments::{Primitive, TypeError},
    shell::variables::VariableType,
    types,
};
use std::iter::Iterator;

#[derive(PartialEq, Clone, Copy, Debug)]
enum IsArrayHelper {
    Valid(usize),
    RootBracketClosed,
    Invalid,
}

/// Determines if the supplied value is either an array or a string.
///
/// - `[ 1 2 3 ]` = Array
/// - `[ 1 2 3 ][1]` = String
/// - `string` = String
pub(crate) fn is_array(value: &str) -> bool {
    if value.starts_with('[') && value.ends_with(']') {
        !value
            .chars()
            .scan(IsArrayHelper::Valid(0), |state, x| {
                // If previous iteration was RootBracketClosed or Invalid then indicate invalid
                if *state == IsArrayHelper::RootBracketClosed || *state == IsArrayHelper::Invalid {
                    *state = IsArrayHelper::Invalid;
                    return Some(*state);
                }

                if x == '[' {
                    if let IsArrayHelper::Valid(open) = *state {
                        *state = IsArrayHelper::Valid(open + 1);
                    }
                } else if x == ']' {
                    if let IsArrayHelper::Valid(open) = *state {
                        *state = IsArrayHelper::Valid(open - 1);
                    }
                }

                // if true, root bracket was closed
                // => any characters after this one indicate invalid array
                if *state == IsArrayHelper::Valid(0) {
                    *state = IsArrayHelper::RootBracketClosed;
                }

                Some(*state)
            })
            .any(|x| x == IsArrayHelper::Invalid)
    } else {
        false
    }
}

pub(crate) fn is_boolean(primitive_type: Primitive, value: &mut small::String) -> Result<&str, TypeError> {
    if ["true", "1", "y"].contains(&&**value) {
        value.clear();
        value.push_str("true");
        Ok(value.as_str())
    } else if ["false", "0", "n"].contains(&&**value) {
        value.clear();
        value.push_str("false");
        Ok(value.as_str())
    } else {
        Err(TypeError::BadValue(primitive_type))
    }
}

fn is_of(primitive_type: Primitive, value: &mut VariableType) -> Result<(), TypeError> {
    if let VariableType::Array(ref mut items) = value {
        match primitive_type {
            Primitive::BooleanArray => {
                for item in items.iter_mut() {
                    is_boolean(primitive_type.clone(), item)?;
                }
                return Ok(());
            }
            Primitive::IntegerArray => {
                let checks_out = items.iter().all(|num| num.parse::<i64>().is_ok());
                if checks_out {
                    return Ok(());
                }
            }
            Primitive::FloatArray => {
                let checks_out = items.iter().all(|num| num.parse::<f64>().is_ok());
                if checks_out {
                    return Ok(());
                }
            }
            _ => unreachable!(),
        }
    } else if let VariableType::Str(ref mut string) = value {
        match primitive_type {
            Primitive::Boolean => {
                is_boolean(primitive_type, string)?;
                return Ok(())
            }
            Primitive::Integer => if string.parse::<i64>().is_ok() {
                return Ok(());
            }
            Primitive::Float => if string.parse::<f64>().is_ok() {
                return Ok(());
            }
            _ => unreachable!(),
        }
    }
    Err(TypeError::BadValue(primitive_type))
}

fn get_string<E: Expander>(shell: &E, value: &str) -> VariableType {
    VariableType::Str(types::Str::from(expand_string(value, shell, false).join(" ")))
}

fn get_array<E: Expander>(shell: &E, value: &str) -> VariableType {
    VariableType::Array(expand_string(value, shell, false))
}

fn get_hash_map<E: Expander>(
    shell: &E,
    expression: &str,
    inner_kind: &Primitive,
) -> Result<VariableType, TypeError> {
    let array = expand_string(expression, shell, false);
    let mut hmap = types::HashMap::with_capacity_and_hasher(array.len(), Default::default());

    for string in array {
        if let Some(found) = string.find('=') {
            let key = &string[..found];
            let value = &string[found + 1..];
            match value_check(shell, value, inner_kind) {
                Ok(VariableType::Str(str_)) => {
                    hmap.insert(key.into(), VariableType::Str(str_));
                }
                Ok(VariableType::Array(array)) => {
                    hmap.insert(key.into(), VariableType::Array(array));
                }
                Ok(VariableType::HashMap(map)) => {
                    hmap.insert(key.into(), VariableType::HashMap(map));
                }
                Ok(VariableType::BTreeMap(map)) => {
                    hmap.insert(key.into(), VariableType::BTreeMap(map));
                }
                Err(type_error) => return Err(type_error),
                _ => (),
            }
        } else {
            return Err(TypeError::BadValue(inner_kind.clone()));
        }
    }

    Ok(VariableType::HashMap(hmap))
}

fn get_btree_map<E: Expander>(
    shell: &E,
    expression: &str,
    inner_kind: &Primitive,
) -> Result<VariableType, TypeError> {
    let array = expand_string(expression, shell, false);
    let mut bmap = types::BTreeMap::new();

    for string in array {
        if let Some(found) = string.find('=') {
            let key = &string[..found];
            let value = &string[found + 1..];
            match value_check(shell, value, inner_kind) {
                Ok(VariableType::Str(str_)) => {
                    bmap.insert(key.into(), VariableType::Str(str_));
                }
                Ok(VariableType::Array(array)) => {
                    bmap.insert(key.into(), VariableType::Array(array));
                }
                Ok(VariableType::HashMap(map)) => {
                    bmap.insert(key.into(), VariableType::HashMap(map));
                }
                Ok(VariableType::BTreeMap(map)) => {
                    bmap.insert(key.into(), VariableType::BTreeMap(map));
                }
                Err(type_error) => return Err(type_error),
                _ => (),
            }
        } else {
            return Err(TypeError::BadValue(inner_kind.clone()));
        }
    }

    Ok(VariableType::BTreeMap(bmap))
}

pub(crate) fn value_check<E: Expander>(
    shell: &E,
    value: &str,
    expected: &Primitive,
) -> Result<VariableType, TypeError> {
    macro_rules! get_string {
        () => {
            get_string(shell, value)
        };
    }
    macro_rules! get_array {
        () => {
            get_array(shell, value)
        };
    }
    let is_array = is_array(value);
    match expected {
        Primitive::Any if is_array => Ok(get_array!()),
        Primitive::Any => Ok(get_string!()),
        Primitive::AnyArray if is_array => Ok(get_array!()),
        Primitive::Str if !is_array => Ok(get_string!()),
        Primitive::StrArray if is_array => Ok(get_array!()),
        Primitive::Boolean | Primitive::Integer | Primitive::Float if !is_array => {
            let mut values = get_string!();
            is_of(expected.clone(), &mut values)?;
            Ok(values)
        }
        Primitive::BooleanArray | Primitive::IntegerArray | Primitive::FloatArray if is_array => {
            let mut values = get_array!();
            is_of(expected.clone(), &mut values)?;
            Ok(values)
        }
        Primitive::HashMap(ref kind) if is_array => get_hash_map(shell, value, kind),
        Primitive::BTreeMap(ref kind) if is_array => get_btree_map(shell, value, kind),
        Primitive::Indexed(_, ref kind) => value_check(shell, value, kind),
        _ => Err(TypeError::BadValue(expected.clone())),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::Array;
    use lexers::TypeError;

    #[test]
    fn is_array_() {
        assert!(is_array("[1 2 3]"));
        assert!(!is_array("[1 2 3][0]"));
        assert!(!is_array("string"));
        assert!(is_array("[1  [2 3]  4 [5 6]]"))
    }

    #[test]
    fn is_boolean_() {
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("1")),     Ok("true"));
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("y")),     Ok("true"));
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("true")),  Ok("true"));
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("0")),     Ok("false"));
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("n")),     Ok("false"));
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("false")), Ok("false"));
        assert_eq!(is_boolean(Primitive::Boolean, &mut small::String::from("other")), Err(TypeError::BadValue(Primitive::Boolean)));
    }

    #[test]
    fn is_integer_array_() {
        assert_eq!(is_of(Primitive::IntegerArray, &mut VariableType::Array(array!["1", "2", "3"])), Ok(()));
        assert_eq!(is_of(Primitive::IntegerArray, &mut VariableType::Array(array!["1", "2", "three"])), Err(TypeError::BadValue(Primitive::IntegerArray)));
    }
}
