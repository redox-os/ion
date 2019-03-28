use super::{super::types, Value};
use itertools::Itertools;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OpError {
    TypeError,
    ParseError(std::num::ParseFloatError),
}

macro_rules! math {
    ($trait:ident, $fn:ident, $op:expr) => {
        math!($trait, $fn, $op, false);
    };
    ($trait:ident, $fn:ident, $op:expr, $allfloat:expr) => {
        impl<'a> std::ops::$trait for &'a Value {
            type Output = Result<Value, OpError>;

            fn $fn(self, rhs: Self) -> Self::Output {
                if let Value::Str(rhs) = rhs {
                    if $allfloat {
                        rhs.parse::<f64>()
                            .map_err(OpError::ParseError)
                            .and_then(|rhs| $op(self, rhs))
                    } else {
                        if let Ok(rhs) = rhs.parse::<i128>() {
                            $op(self, rhs)
                        } else {
                            rhs.parse::<f64>()
                                .map_err(OpError::ParseError)
                                .and_then(|rhs| $op(self, rhs))
                        }
                    }
                } else {
                    Err(OpError::TypeError)
                }
            }
        }

        impl<'a> std::ops::$trait<i128> for &'a Value {
            type Output = Result<Value, OpError>;

            fn $fn(self, rhs: i128) -> Self::Output {
                match self {
                    Value::Str(lhs) => {
                        if $allfloat {
                            lhs.parse::<f64>()
                                .map_err(OpError::ParseError)
                                .map(|lhs| $op(lhs, rhs as f64).to_string())
                                .map(|v| Value::Str(v.into()))
                        } else {
                            lhs.parse::<i128>()
                                .map(|lhs| $op(lhs, rhs).to_string())
                                .or_else(|_| {
                                    lhs.parse::<f64>()
                                        .map_err(OpError::ParseError)
                                        .map(|lhs| $op(lhs, rhs as f64).to_string())
                                })
                                .map(|v| Value::Str(v.into()))
                        }
                    }
                    Value::Array(lhs) => lhs
                        .iter()
                        // When array will contain values insted of strings, clone will no longer be
                        // needed
                        .map(|el| $op(&Value::Str(el.clone()), rhs))
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

        impl<'a> std::ops::$trait<f64> for &'a Value {
            type Output = Result<Value, OpError>;

            fn $fn(self, rhs: f64) -> Self::Output {
                match self {
                    Value::Str(lhs) => lhs
                        .parse::<f64>()
                        .map_err(OpError::ParseError)
                        .map(|lhs| $op(lhs, rhs).to_string())
                        .map(|v| Value::Str(v.into())),
                    Value::Array(lhs) => lhs
                        .iter()
                        // When array will contain values insted of strings, clone will no longer be
                        // needed
                        .map(|el| $op(&Value::Str(el.clone()), rhs))
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
    };
}

math!(Add, add, |lhs, rhs| { lhs + rhs });
math!(Sub, sub, |lhs, rhs| { lhs - rhs });
math!(Mul, mul, |lhs, rhs| { lhs * rhs });
math!(Div, div, |lhs, rhs| { lhs / rhs }, true);
