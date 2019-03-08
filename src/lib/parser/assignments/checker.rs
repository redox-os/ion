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

pub(crate) fn as_boolean(value: &mut small::String) -> &str {
    if ["true", "1", "y"].contains(&&**value) {
        value.clear();
        value.push_str("true");
    } else if ["false", "0", "n"].contains(&&**value) {
        value.clear();
        value.push_str("false");
    } else {
        value.clear();
        value.push_str("invalid");
    }
    value.as_str()
}

fn is_expected_with(expected_type: Primitive, value: &mut VariableType) -> Result<(), TypeError> {
    let checks_out = if let VariableType::Array(ref mut items) = value {
            match expected_type {
                Primitive::BooleanArray => items.iter_mut().all(|item| ["true", "false"].contains(&as_boolean(item))),
                Primitive::IntegerArray => items.iter().all(|num| num.parse::<i64>().is_ok()),
                Primitive::FloatArray => items.iter().all(|num| num.parse::<f64>().is_ok()),
                _ => false,
            }
        } else if let VariableType::Str(ref mut string) = value {
            match expected_type {
                Primitive::Boolean => ["true", "false"].contains(&as_boolean(string)),
                Primitive::Integer => string.parse::<i64>().is_ok(),
                Primitive::Float => string.parse::<f64>().is_ok(),
                _ => false,
            }
        } else {
            false
        };

    if checks_out {
        return Ok(());
    }
    Err(TypeError::BadValue(expected_type))
}

fn get_map_of<E: Expander>(
    primitive_type: &Primitive,
    shell: &E,
    expression: &str,
) -> Result<VariableType, TypeError> {
    let array = expand_string(expression, shell, false);

    let inner = match primitive_type {
        Primitive::HashMap(ref inner) => inner,
        Primitive::BTreeMap(ref inner) => inner,
        _ => unreachable!(),
    };

    let iter = array.iter().map(|string| {
        if let Some(found) = string.find('=') {
            let key = &string[..found];
            let value = value_check(shell, &string[found + 1..], inner)?;
            match value {
                VariableType::Str(_) | VariableType::Array(_) | VariableType::HashMap(_) | VariableType::BTreeMap(_) => return Ok((key.into(), value)),
                _ => return Err(TypeError::BadValue((**inner).clone())),
            }
        }
        Err(TypeError::BadValue((**inner).clone()))
    });

    match primitive_type {
        Primitive::HashMap(_) => {
            let mut hmap = types::HashMap::with_capacity_and_hasher(array.len(), Default::default());
            for item in iter {
                let (key, value) = item?;
                hmap.insert(key, value);
            }
            Ok(VariableType::HashMap(hmap))
        }
        Primitive::BTreeMap(_) => {
            let mut bmap = types::BTreeMap::new();
            for item in iter {
                let (key, value) = item?;
                bmap.insert(key, value);
            }
            Ok(VariableType::BTreeMap(bmap))
        }
        _ => unreachable!(),
    }
}

pub(crate) fn value_check<E: Expander>(
    shell: &E,
    value: &str,
    expected: &Primitive,
) -> Result<VariableType, TypeError> {
    let mut extracted = if is_array(value) {
            shell.get_array(value)
        } else {
            shell.get_string(value)
        };
    match expected {
        Primitive::Any | Primitive::Str | Primitive::AnyArray | Primitive::StrArray => {
            Ok(extracted)
        }
        Primitive::Boolean | Primitive::Integer | Primitive::Float |
        Primitive::BooleanArray | Primitive::IntegerArray | Primitive::FloatArray => {
            is_expected_with(expected.clone(), &mut extracted)?;
            Ok(extracted)
        }
        Primitive::HashMap(_) | Primitive::BTreeMap(_) => get_map_of(expected, shell, value),
        Primitive::Indexed(_, ref kind) => value_check(shell, value, kind),
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
        assert_eq!(as_boolean(&mut small::String::from("1")),     "true");
        assert_eq!(as_boolean(&mut small::String::from("y")),     "true");
        assert_eq!(as_boolean(&mut small::String::from("true")),  "true");
        assert_eq!(as_boolean(&mut small::String::from("0")),     "false");
        assert_eq!(as_boolean(&mut small::String::from("n")),     "false");
        assert_eq!(as_boolean(&mut small::String::from("false")), "false");
        assert_eq!(as_boolean(&mut small::String::from("other")), "invalid");
    }

    #[test]
    fn is_integer_array_() {
        assert_eq!(is_expected_with(Primitive::IntegerArray, &mut VariableType::Array(array!["1", "2", "3"])), Ok(()));
        assert_eq!(is_expected_with(Primitive::IntegerArray, &mut VariableType::Array(array!["1", "2", "three"])), Err(TypeError::BadValue(Primitive::IntegerArray)));
    }
}
