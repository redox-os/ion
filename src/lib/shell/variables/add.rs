use super::{super::types, Value};
use itertools::Itertools;
use std::ops::Add;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OpError {
    TypeError,
    ParseError(std::num::ParseFloatError),
}

impl<'a> Add for &'a Value {
    type Output = Result<Value, OpError>;

    fn add(self, rhs: Self) -> Self::Output {
        if let Value::Str(rhs) = rhs {
            if let Ok(rhs) = rhs.parse::<i128>() {
                self + rhs
            } else {
                rhs.parse::<f64>().map_err(OpError::ParseError).and_then(|rhs| self + rhs)
            }
        } else {
            Err(OpError::TypeError)
        }
    }
}

impl<'a> Add<i128> for &'a Value {
    type Output = Result<Value, OpError>;

    fn add(self, rhs: i128) -> Self::Output {
        match self {
            Value::Str(lhs) => lhs
                .parse::<i128>()
                .map(|lhs| (lhs + rhs).to_string())
                .or_else(|_| {
                    lhs.parse::<f64>()
                        .map_err(OpError::ParseError)
                        .map(|lhs| (lhs + rhs as f64).to_string())
                })
                .map(|v| Value::Str(v.into())),
            Value::Array(lhs) => lhs
                .iter()
                // When array will contain values insted of strings, clone will no longer be needed
                .map(|el| &Value::Str(el.clone()) + rhs)
                .map_results(|result| {
                    if let Value::Str(res) = result {
                        res
                    } else {
                        unreachable!();
                    }
                })
                .collect::<Result<types::Array, _>>()
                .map(Value::Array),
            _ => Err(OpError::TypeError),
        }
    }
}

impl<'a> Add<f64> for &'a Value {
    type Output = Result<Value, OpError>;

    fn add(self, rhs: f64) -> Self::Output {
        match self {
            Value::Str(lhs) => lhs
                .parse::<f64>()
                .map_err(OpError::ParseError)
                .map(|lhs| (lhs + rhs).to_string())
                .map(|v| Value::Str(v.into())),
            Value::Array(lhs) => lhs
                .iter()
                // When array will contain values insted of strings, clone will no longer be needed
                .map(|el| &Value::Str(el.clone()) + rhs)
                .map_results(|result| {
                    if let Value::Str(res) = result {
                        res
                    } else {
                        unreachable!();
                    }
                })
                .collect::<Result<types::Array, _>>()
                .map(Value::Array),
            _ => Err(OpError::TypeError),
        }
    }
}

#[cfg(test)]
mod test {
    use super::super::{super::types, Value};

    #[test]
    fn add_integer_integer() {
        let a = Value::Str("1".into());
        assert_eq!(&a + 2, Ok(Value::Str("3".into())));
        assert_eq!(&a + -2, Ok(Value::Str("-1".into())));
        assert_eq!(&a + 0, Ok(Value::Str("1".into())));
    }

    #[test]
    fn add_float_integer() {
        let a = Value::Str("1.2".into());
        assert_eq!(&a + 2, Ok(Value::Str("3.2".into())));
        assert_eq!(&a + -2, Ok(Value::Str("-0.8".into())));
        assert_eq!(&a + 0, Ok(Value::Str("1.2".into())));
    }

    #[test]
    fn add_integer_float() {
        let a = Value::Str("1".into());
        assert_eq!(&a + 2.3, Ok(Value::Str("3.3".into())));
        // Floating point artifacts
        assert_eq!(&a + -2.3, Ok(Value::Str("-1.2999999999999998".into())));
        assert_eq!(&a + 0., Ok(Value::Str("1".into())));
    }

    #[test]
    fn add_float_float() {
        let a = Value::Str("1.2".into());
        assert_eq!(&a + 2.8, Ok(Value::Str("4".into())));
        // Floating point artifacts
        assert_eq!(&a + -2.2, Ok(Value::Str("-1.0000000000000002".into())));
        assert_eq!(&a + 0, Ok(Value::Str("1.2".into())));
    }

    #[test]
    fn add_array_integer() {
        let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
        assert_eq!(
            &a + 2,
            Ok(Value::Array(array![types::Str::from("3.2"), types::Str::from("3")]))
        );
    }

    #[test]
    fn add_array_float() {
        let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
        assert_eq!(
            &a + 2.8,
            Ok(Value::Array(array![types::Str::from("4"), types::Str::from("3.8")]))
        );
    }

    #[test]
    fn add_var_var_str() {
        let a = Value::Str("1.2".into());
        assert_eq!(&a + &Value::Str("2.8".into()), Ok(Value::Str("4".into())));
        assert_eq!(&a + &Value::Str("2".into()), Ok(Value::Str("3.2".into())));
    }

    #[test]
    fn add_var_var_array() {
        let a = Value::Array(array![types::Str::from("1.2"), types::Str::from("1")]);
        assert_eq!(
            &a + &Value::Str("2.8".into()),
            Ok(Value::Array(array![types::Str::from("4"), types::Str::from("3.8")]))
        );
    }
}
