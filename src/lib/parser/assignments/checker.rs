use super::super::Expander;
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
pub fn is_array(value: &str) -> bool {
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

pub fn is_boolean(value: &mut types::Str) -> bool {
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

fn get_map_of<E: Expander>(
    primitive_type: &Primitive,
    shell: &E,
    expression: &str,
) -> Result<Value<'static>, TypeError> {
    let array = shell.expand_string(expression);

    let inner_kind = match primitive_type {
        Primitive::HashMap(ref inner) => inner,
        Primitive::BTreeMap(ref inner) => inner,
        _ => unreachable!(),
    };

    let size = array.len();

    let iter = array.into_iter().map(|string| {
        let mut parts = string.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            value_check(shell, value, inner_kind).and_then(|val| match val {
                Value::Str(_) | Value::Array(_) | Value::HashMap(_) | Value::BTreeMap(_) => {
                    Ok(((*key).into(), val))
                }
                _ => Err(TypeError::BadValue((**inner_kind).clone())),
            })
        } else {
            Err(TypeError::BadValue(*inner_kind.clone()))
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

pub fn value_check<E: Expander>(
    shell: &E,
    value: &str,
    expected: &Primitive,
) -> Result<Value<'static>, TypeError> {
    if is_array(value) {
        let extracted = shell.get_array(value);
        match expected {
            Primitive::StrArray | Primitive::Str => extracted
                .iter()
                .map(|item| value_check(shell, item, &Primitive::Str))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Primitive::BooleanArray => extracted
                .iter()
                .map(|item| value_check(shell, item, &Primitive::Boolean))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Primitive::IntegerArray => extracted
                .iter()
                .map(|item| value_check(shell, item, &Primitive::Integer))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Primitive::FloatArray => extracted
                .iter()
                .map(|item| value_check(shell, item, &Primitive::Float))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Primitive::HashMap(_) | Primitive::BTreeMap(_) => get_map_of(expected, shell, value),
            Primitive::Indexed(_, ref kind) => value_check(shell, value, kind),
            _ => Err(TypeError::BadValue(expected.clone())),
        }
    } else {
        let mut extracted = shell.get_string(value);
        match expected {
            Primitive::Str => Ok(Value::Str(extracted)),
            Primitive::Boolean => {
                if is_boolean(&mut extracted) {
                    Ok(Value::Str(extracted))
                } else {
                    Err(TypeError::BadValue(expected.clone()))
                }
            }
            Primitive::Integer if extracted.parse::<i64>().is_ok() => Ok(Value::Str(extracted)),
            Primitive::Float if extracted.parse::<f64>().is_ok() => Ok(Value::Str(extracted)),
            Primitive::Indexed(_, ref kind) => value_check(shell, value, kind),
            _ => Err(TypeError::BadValue(expected.clone())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use lexers::TypeError;

    struct VariableExpander;

    impl Expander for VariableExpander {
        fn get_string(&self, variable: &str) -> types::Str { variable.into() }

        fn get_array(&self, variable: &str) -> types::Args {
            variable[1..variable.len() - 1].split(' ').map(Into::into).collect()
        }
    }

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
            value_check(&VariableExpander, "[1 2 3]", &Primitive::IntegerArray),
            Ok(Value::Array(vec![
                Value::Str("1".into()),
                Value::Str("2".into()),
                Value::Str("3".into())
            ]))
        );
        assert_eq!(
            value_check(&VariableExpander, "[1 2 three]", &Primitive::IntegerArray),
            Err(TypeError::BadValue(Primitive::Integer))
        );
    }
}
