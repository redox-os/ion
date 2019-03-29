use super::{super::types, Value};
use itertools::Itertools;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OpError {
    TypeError,
    ParseError(std::num::ParseFloatError),
}

pub trait Pow<RHS = Self> {
    type Output;

    fn pow(self, power: RHS) -> Self::Output;
}

macro_rules! math {
    ($trait:ident, $fn:ident) => {
        math!($trait, $fn, false);
    };
    ($trait:ident, $fn:ident, $allfloat:expr) => {
        math!(
            $trait,
            $fn,
            |lhs: f64, rhs: f64| { lhs.$fn(rhs) },
            |lhs: i128, rhs: i128| { lhs.$fn(rhs) },
            true
        );
    };
    ($trait:ident, $fn:ident, $op_f_f:expr, $op_i_i:expr, $allfloat:expr) => {
        impl<'a> $trait for &'a Value {
            type Output = Result<Value, OpError>;

            fn $fn(self, rhs: Self) -> Self::Output {
                if let Value::Str(rhs) = rhs {
                    if $allfloat {
                        rhs.parse::<f64>()
                            .map_err(OpError::ParseError)
                            .and_then(|rhs| self.$fn(rhs))
                    } else {
                        if let Ok(rhs) = rhs.parse::<i128>() {
                            self.$fn(rhs)
                        } else {
                            rhs.parse::<f64>()
                                .map_err(OpError::ParseError)
                                .and_then(|rhs| self.$fn(rhs))
                        }
                    }
                } else {
                    Err(OpError::TypeError)
                }
            }
        }

        impl<'a> $trait<i128> for &'a Value {
            type Output = Result<Value, OpError>;

            fn $fn(self, rhs: i128) -> Self::Output {
                match self {
                    Value::Str(lhs) => {
                        if $allfloat {
                            lhs.parse::<f64>()
                                .map_err(OpError::ParseError)
                                .map(|lhs| $op_f_f(lhs, rhs as f64).to_string())
                                .map(|v| Value::Str(v.into()))
                        } else {
                            lhs.parse::<i128>()
                                .map(|lhs| $op_i_i(lhs, rhs).to_string())
                                .or_else(|_| {
                                    lhs.parse::<f64>()
                                        .map_err(OpError::ParseError)
                                        .map(|lhs| $op_f_f(lhs, rhs as f64).to_string())
                                })
                                .map(|v| Value::Str(v.into()))
                        }
                    }
                    Value::Array(lhs) => lhs
                        .iter()
                        // When array will contain values insted of strings, clone will no longer be
                        // needed
                        .map(|el| Value::Str(el.clone()).$fn(rhs))
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

        impl<'a> $trait<f64> for &'a Value {
            type Output = Result<Value, OpError>;

            fn $fn(self, rhs: f64) -> Self::Output {
                match self {
                    Value::Str(lhs) => lhs
                        .parse::<f64>()
                        .map_err(OpError::ParseError)
                        .map(|lhs| $op_f_f(lhs, rhs).to_string())
                        .map(|v| Value::Str(v.into())),
                    Value::Array(lhs) => lhs
                        .iter()
                        // When array will contain values insted of strings, clone will no longer be
                        // needed
                        .map(|el| Value::Str(el.clone()).$fn(rhs))
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

math!(Add, add);
math!(Sub, sub);
math!(Mul, mul);
math!(Div, div, true);
math!(
    Pow,
    pow,
    |lhs: f64, rhs: f64| { lhs.powf(rhs) },
    |lhs: i128, rhs: i128| { lhs.pow(rhs as u32) },
    true
);
