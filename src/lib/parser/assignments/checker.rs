use super::super::{expand_string, Expander};
use lexers::assignments::{Primitive, TypeError};
use shell::variables::VariableType;
use std::iter::Iterator;
use types;

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

pub(crate) fn is_boolean(value: &str) -> Result<&str, ()> {
    if ["true", "1", "y"].contains(&value) {
        Ok("true")
    } else if ["false", "0", "n"].contains(&value) {
        Ok("false")
    } else {
        Err(())
    }
}

fn is_boolean_string(value: &VariableType) -> Result<&str, ()> {
    if let VariableType::Str(ref value) = *value {
        is_boolean(&value.as_str())
    } else {
        unreachable!()
    }
}

fn is_integer_string(value: VariableType) -> Result<VariableType, ()> {
    let is_ok = if let VariableType::Str(ref num) = value {
        num.parse::<i64>().is_ok()
    } else {
        unreachable!()
    };

    if is_ok {
        Ok(value)
    } else {
        Err(())
    }
}

fn is_float_string(value: VariableType) -> Result<VariableType, ()> {
    let is_ok = if let VariableType::Str(ref num) = value {
        num.parse::<f64>().is_ok()
    } else {
        unreachable!()
    };

    if is_ok {
        Ok(value)
    } else {
        Err(())
    }
}

fn is_boolean_array(values: &mut VariableType) -> bool {
    if let VariableType::Array(ref mut values) = *values {
        for element in values.iter_mut() {
            let boolean = {
                match is_boolean(&element) {
                    Ok(boolean) => boolean.into(),
                    Err(()) => return false,
                }
            };
            *element = boolean;
        }
        true
    } else {
        unreachable!()
    }
}

fn is_integer_array(value: VariableType) -> Result<VariableType, ()> {
    let is_ok = if let VariableType::Array(ref nums) = value {
        nums.iter().all(|num| num.parse::<i64>().is_ok())
    } else {
        unreachable!()
    };

    if is_ok {
        Ok(value)
    } else {
        Err(())
    }
}

fn is_float_array(value: VariableType) -> Result<VariableType, ()> {
    let is_ok = if let VariableType::Array(ref nums) = value {
        nums.iter().all(|num| num.parse::<f64>().is_ok())
    } else {
        unreachable!()
    };

    if is_ok {
        Ok(value)
    } else {
        Err(())
    }
}

fn get_string<E: Expander>(shell: &E, value: &str) -> VariableType {
    VariableType::Str(types::Str::from(
        expand_string(value, shell, false).join(" "),
    ))
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
        Primitive::Boolean if !is_array => {
            let value = get_string!();
            let value =
                is_boolean_string(&value).map_err(|_| TypeError::BadValue(expected.clone()))?;
            Ok(VariableType::Str(value.into()))
        }
        Primitive::BooleanArray if is_array => {
            let mut values = get_array!();
            if is_boolean_array(&mut values) {
                Ok(values)
            } else {
                Err(TypeError::BadValue(expected.clone()))
            }
        }
        Primitive::Integer if !is_array => {
            is_integer_string(get_string!()).map_err(|_| TypeError::BadValue(expected.clone()))
        }
        Primitive::IntegerArray if is_array => {
            is_integer_array(get_array!()).map_err(|_| TypeError::BadValue(expected.clone()))
        }
        Primitive::Float if !is_array => {
            is_float_string(get_string!()).map_err(|_| TypeError::BadValue(expected.clone()))
        }
        Primitive::FloatArray if is_array => {
            is_float_array(get_array!()).map_err(|_| TypeError::BadValue(expected.clone()))
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
    use types::Array;

    #[test]
    fn is_array_() {
        assert!(is_array("[1 2 3]"));
        assert!(!is_array("[1 2 3][0]"));
        assert!(!is_array("string"));
        assert!(is_array("[1  [2 3]  4 [5 6]]"))
    }

    #[test]
    fn is_boolean_() {
        assert_eq!(is_boolean("1"), Ok("true"));
        assert_eq!(is_boolean("y"), Ok("true"));
        assert_eq!(is_boolean("true"), Ok("true"));
        assert_eq!(is_boolean("0"), Ok("false"));
        assert_eq!(is_boolean("n"), Ok("false"));
        assert_eq!(is_boolean("false"), Ok("false"));
        assert_eq!(is_boolean("other"), Err(()));
    }

    #[test]
    fn is_integer_array_() {
        let expected = Ok(VariableType::Array(array!["1", "2", "3"]));
        assert_eq!(
            is_integer_array(VariableType::Array(array!["1", "2", "3"])),
            expected
        );
        assert_eq!(
            is_integer_array(VariableType::Array(array!["1", "2", "three"])),
            Err(())
        );
    }
}
