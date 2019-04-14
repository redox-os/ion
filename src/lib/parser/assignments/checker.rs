use super::super::{expand_string, Expander};
use crate::{
    lexers::assignments::{Primitive, TypeError},
    shell::variables::Value,
    types,
};
use std::iter::Iterator;

/// Determines if the supplied value is either an array or a string.
///
/// - `[ 1 2 3 ]` = Array
/// - `[ 1 2 3 ][1]` = String
/// - `string` = String
pub(crate) fn is_array(value: &str) -> bool {
    if value.ends_with(']') {
        let mut brackets = value.chars().scan(0, |state, c| {
            *state += match c {
                '[' => 1,
                ']' => -1,
                _ => 0,
            };
            Some(*state)
        });
        // final bracket should be the last char
        brackets.any(|x| x == 0) && brackets.next().is_none()
    } else {
        false
    }
}

pub(crate) fn is_boolean(value: &mut types::Str) -> bool {
    if ["true", "1", "y"].contains(&value.as_str()) {
        value.clear();
        value.push_str("true");
        true
    } else if ["false", "0", "n"].contains(&value.as_str()) {
        value.clear();
        value.push_str("false");
        true
    } else {
        false
    }
}

fn is_expected_with(expected_type: Primitive, value: &mut Value) -> Result<(), TypeError> {
    let checks_out = if let Value::Array(ref mut items) = value {
        match expected_type {
            Primitive::BooleanArray => items.iter_mut().all(|item| {
                is_expected_with(Primitive::Boolean, &mut Value::Str(item.to_owned())).is_ok()
            }),
            Primitive::IntegerArray => items.iter_mut().all(|item| {
                is_expected_with(Primitive::Integer, &mut Value::Str(item.to_owned())).is_ok()
            }),
            Primitive::FloatArray => items.iter_mut().all(|item| {
                is_expected_with(Primitive::Float, &mut Value::Str(item.to_owned())).is_ok()
            }),
            _ => false,
        }
    } else if let Value::Str(ref mut string) = value {
        match expected_type {
            Primitive::Boolean => is_boolean(string),
            Primitive::Integer => string.parse::<i64>().is_ok(),
            Primitive::Float => string.parse::<f64>().is_ok(),
            _ => false,
        }
    } else {
        false
    };

    if checks_out {
        Ok(())
    } else {
        Err(TypeError::BadValue(expected_type))
    }
}

fn get_map_of<E: Expander>(
    primitive_type: &Primitive,
    shell: &E,
    expression: &str,
) -> Result<Value, TypeError> {
    let array = expand_string(expression, shell);

    let inner_kind = match primitive_type {
        Primitive::HashMap(ref inner) => inner,
        Primitive::BTreeMap(ref inner) => inner,
        _ => unreachable!(),
    };

    let size = array.len();

    let iter = array.into_iter().map(|string| {
        match string.splitn(2, '=').collect::<Vec<_>>().as_slice() {
            [key, value] => value_check(shell, value, inner_kind).and_then(|val| match val {
                Value::Str(_) | Value::Array(_) | Value::HashMap(_) | Value::BTreeMap(_) => {
                    Ok(((*key).into(), val))
                }
                _ => Err(TypeError::BadValue((**inner_kind).clone())),
            }),
            _ => Err(TypeError::BadValue(*inner_kind.clone())),
        }
    });

    match primitive_type {
        Primitive::HashMap(_) => {
            let mut hmap = types::HashMap::with_capacity_and_hasher(size, Default::default());
            for item in iter {
                let (key, value) = item?;
                hmap.insert(key, value);
            }
            Ok(Value::HashMap(hmap))
        }
        Primitive::BTreeMap(_) => {
            let mut bmap = types::BTreeMap::new();
            for item in iter {
                let (key, value) = item?;
                bmap.insert(key, value);
            }
            Ok(Value::BTreeMap(bmap))
        }
        _ => unreachable!(),
    }
}

pub(crate) fn value_check<E: Expander>(
    shell: &E,
    value: &str,
    expected: &Primitive,
) -> Result<Value, TypeError> {
    let mut extracted =
        if is_array(value) { shell.get_array(value) } else { shell.get_string(value) };
    match expected {
        Primitive::Str | Primitive::StrArray => Ok(extracted),
        Primitive::Boolean
        | Primitive::Integer
        | Primitive::Float
        | Primitive::BooleanArray
        | Primitive::IntegerArray
        | Primitive::FloatArray => {
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
        let mut test: small::String = "1".into();
        assert!(is_boolean(&mut test));
        assert_eq!(test, "true");
        test = small::String::from("y");
        assert!(is_boolean(&mut test));
        assert_eq!(test, "true");
        test = small::String::from("true");
        assert!(is_boolean(&mut test));
        assert_eq!(test, "true");

        test = small::String::from("0");
        assert!(is_boolean(&mut test));
        assert_eq!(test, "false");
        test = small::String::from("n");
        assert!(is_boolean(&mut test));
        assert_eq!(test, "false");
        test = small::String::from("false");
        assert!(is_boolean(&mut test));
        assert_eq!(test, "false");

        test = small::String::from("other");
        assert!(!is_boolean(&mut test));
        assert_eq!(test, "other");
    }

    #[test]
    fn is_integer_array_() {
        assert_eq!(
            is_expected_with(Primitive::IntegerArray, &mut Value::Array(array!["1", "2", "3"])),
            Ok(())
        );
        assert_eq!(
            is_expected_with(Primitive::IntegerArray, &mut Value::Array(array!["1", "2", "three"])),
            Err(TypeError::BadValue(Primitive::IntegerArray))
        );
    }
}
