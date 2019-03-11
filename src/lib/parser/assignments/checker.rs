use super::super::{expand_string, Expander};
use crate::{
    lexers::assignments::{Primitive, TypeError},
    shell::variables::VariableType,
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

pub(crate) fn to_boolean(value: &mut types::Str) -> Result<(), ()> {
    if ["true", "1", "y"].contains(&value.as_str()) {
        value.clear();
        value.push_str("true");
        Ok(())
    } else if ["false", "0", "n"].contains(&value.as_str()) {
        value.clear();
        value.push_str("false");
        Ok(())
    } else {
        Err(())
    }
}

fn is_expected_with(expected_type: Primitive, value: &mut VariableType) -> Result<(), TypeError> {
    let checks_out = if let VariableType::Array(ref mut items) = value {
        match expected_type {
            Primitive::BooleanArray => items.iter_mut().all(|item| {
                is_expected_with(Primitive::Boolean, &mut VariableType::Str(item.to_owned()))
                    .is_ok()
            }),
            Primitive::IntegerArray => items.iter_mut().all(|item| {
                is_expected_with(Primitive::Integer, &mut VariableType::Str(item.to_owned()))
                    .is_ok()
            }),
            Primitive::FloatArray => items.iter_mut().all(|item| {
                is_expected_with(Primitive::Float, &mut VariableType::Str(item.to_owned())).is_ok()
            }),
            _ => false,
        }
    } else if let VariableType::Str(ref mut string) = value {
        match expected_type {
            Primitive::Boolean => to_boolean(string).is_ok(),
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
) -> Result<VariableType, TypeError> {
    let array = expand_string(expression, shell, false);

    let inner_kind = match primitive_type {
        Primitive::HashMap(ref inner) => inner,
        Primitive::BTreeMap(ref inner) => inner,
        _ => unreachable!(),
    };

    let size = array.len();

    let iter = array.into_iter().map(|string| {
        match string.splitn(2, '=').collect::<Vec<_>>().as_slice() {
            [key, value] => value_check(shell, value, inner_kind).and_then(|val| match val {
                VariableType::Str(_)
                | VariableType::Array(_)
                | VariableType::HashMap(_)
                | VariableType::BTreeMap(_) => Ok(((*key).into(), val)),
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
        let mut test = small::String::from("1");
        to_boolean(&mut test).unwrap();
        assert_eq!(test, small::String::from("true"));
        let mut test = small::String::from("y");
        to_boolean(&mut test).unwrap();
        assert_eq!(test, small::String::from("true"));
        let mut test = small::String::from("true");
        to_boolean(&mut test).unwrap();
        assert_eq!(test, small::String::from("true"));

        let mut test = small::String::from("0");
        to_boolean(&mut test).unwrap();
        assert_eq!(test, small::String::from("false"));
        let mut test = small::String::from("n");
        to_boolean(&mut test).unwrap();
        assert_eq!(test, small::String::from("false"));
        let mut test = small::String::from("false");
        to_boolean(&mut test).unwrap();
        assert_eq!(test, small::String::from("false"));

        assert_eq!(to_boolean(&mut small::String::from("1")), Ok(()));
        assert_eq!(to_boolean(&mut small::String::from("y")), Ok(()));
        assert_eq!(to_boolean(&mut small::String::from("true")), Ok(()));
        assert_eq!(to_boolean(&mut small::String::from("0")), Ok(()));
        assert_eq!(to_boolean(&mut small::String::from("n")), Ok(()));
        assert_eq!(to_boolean(&mut small::String::from("false")), Ok(()));
        assert_eq!(to_boolean(&mut small::String::from("other")), Err(()));
    }

    #[test]
    fn is_integer_array_() {
        assert_eq!(
            is_expected_with(
                Primitive::IntegerArray,
                &mut VariableType::Array(array!["1", "2", "3"])
            ),
            Ok(())
        );
        assert_eq!(
            is_expected_with(
                Primitive::IntegerArray,
                &mut VariableType::Array(array!["1", "2", "three"])
            ),
            Err(TypeError::BadValue(Primitive::IntegerArray))
        );
    }
}
