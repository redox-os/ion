use super::{super::types, Value};
use itertools::Itertools;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OpError {
    TypeError,
    CalculationError,
    ParseError(std::num::ParseFloatError),
}

pub trait Pow<RHS = Self> {
    type Output;

    fn pow(self, power: RHS) -> Self::Output;
}

macro_rules! math {
    ($trait:ident, $fn:ident, $op_f_f:expr, $op_i_i:expr) => {
        math!($trait, $fn, $op_f_f, $op_i_i, false);
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
                    Value::Str(lhs) => if $allfloat {
                        lhs.parse::<f64>()
                            .map_err(OpError::ParseError)
                            .map(|lhs| $op_f_f(lhs, rhs as f64).to_string())
                    } else {
                        if let Ok(lhs) = lhs.parse::<i128>() {
                            $op_i_i(lhs, rhs)
                                .ok_or(OpError::CalculationError)
                                .map(|result| result.to_string())
                        } else {
                            lhs.parse::<f64>()
                                .map_err(OpError::ParseError)
                                .map(|lhs| $op_f_f(lhs, rhs as f64).to_string())
                        }
                    }
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

math!(Add, add, |lhs: f64, rhs: f64| { lhs.add(rhs) }, |lhs: i128, rhs: i128| {
    lhs.checked_add(rhs)
});
math!(Sub, sub, |lhs: f64, rhs: f64| { lhs.sub(rhs) }, |lhs: i128, rhs: i128| {
    lhs.checked_sub(rhs)
});
math!(Mul, mul, |lhs: f64, rhs: f64| { lhs.mul(rhs) }, |lhs: i128, rhs: i128| {
    lhs.checked_mul(rhs)
});
math!(
    Div,
    div,
    |lhs: f64, rhs: f64| { lhs.div(rhs) },
    |lhs: i128, rhs: i128| { lhs.checked_div(rhs) },
    true
);
// checked pow will only be available with version 1.34, so for now, only perform operation
math!(
    Pow,
    pow,
    |lhs: f64, rhs: f64| { lhs.powf(rhs) },
    |lhs: i128, rhs: i128| { Some(lhs.pow(rhs as u32)) },
    true
);
