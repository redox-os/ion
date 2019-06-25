use super::Value;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OpError {
    TypeError,
    CalculationError,
    ParseError(lexical::Error),
}

pub trait Pow<RHS = Self> {
    type Output;

    fn pow(self, power: RHS) -> Self::Output;
}

pub trait EuclDiv<RHS = Self> {
    type Output;

    fn eucl_div(self, rhs: RHS) -> Self::Output;
}

macro_rules! math {
    ($trait:ident, $fn:ident, $op_f_f:expr, $op_i_i:expr) => {
        math!($trait, $fn, $op_f_f, $op_i_i, false);
    };
    ($trait:ident, $fn:ident, $op_f_f:expr, $op_i_i:expr, $allfloat:expr) => {
        impl<'a, T> $trait for &'a Value<T> {
            type Output = Result<Value<T>, OpError>;

            fn $fn(self, rhs: Self) -> Self::Output {
                if let Value::Str(rhs) = rhs {
                    if $allfloat {
                        lexical::try_parse::<f64, _>(rhs)
                            .map_err(OpError::ParseError)
                            .and_then(|rhs| self.$fn(rhs))
                    } else {
                        if let Ok(rhs) = lexical::try_parse::<i128, _>(rhs) {
                            self.$fn(rhs)
                        } else {
                            lexical::try_parse::<f64, _>(rhs)
                                .map_err(OpError::ParseError)
                                .and_then(|rhs| self.$fn(rhs))
                        }
                    }
                } else {
                    Err(OpError::TypeError)
                }
            }
        }

        impl<'a, T> $trait<Value<T>> for &'a Value<T> {
            type Output = Result<Value<T>, OpError>;

            fn $fn(self, rhs: Value<T>) -> Self::Output {
                self.$fn(&rhs)
            }
        }

        impl<'a, T> $trait<i128> for &'a Value<T> {
            type Output = Result<Value<T>, OpError>;

            fn $fn(self, rhs: i128) -> Self::Output {
                match self {
                    Value::Str(lhs) => if $allfloat {
                        lexical::try_parse::<f64, _>(lhs)
                            .map_err(OpError::ParseError)
                            .map(|lhs| lexical::to_string($op_f_f(lhs, rhs as f64)))
                    } else {
                        if let Ok(lhs) = lexical::try_parse::<i128, _>(lhs) {
                            $op_i_i(lhs, rhs)
                                .ok_or(OpError::CalculationError)
                                .map(lexical::to_string)
                        } else {
                            lexical::try_parse::<f64, _>(lhs)
                                .map_err(OpError::ParseError)
                                .map(|lhs| lexical::to_string($op_f_f(lhs, rhs as f64)))
                        }
                    }
                    .map(Value::from),
                    Value::Array(lhs) => {
                        lhs.iter().map(|el| el.$fn(rhs)).collect::<Result<Value<T>, _>>()
                    }
                    _ => Err(OpError::TypeError),
                }
            }
        }

        impl<'a, T> $trait<f64> for &'a Value<T> {
            type Output = Result<Value<T>, OpError>;

            fn $fn(self, rhs: f64) -> Self::Output {
                match self {
                    Value::Str(lhs) => lexical::try_parse::<f64, _>(lhs)
                        .map_err(OpError::ParseError)
                        .map(|lhs| lexical::to_string($op_f_f(lhs, rhs)))
                        .map(Value::from),
                    Value::Array(lhs) => {
                        lhs.iter().map(|el| el.$fn(rhs)).collect::<Result<Value<T>, _>>()
                    }
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
math!(EuclDiv, eucl_div, |lhs: f64, rhs: f64| { (lhs / rhs) as i128 }, |lhs: i128, rhs: i128| {
    lhs.checked_div(rhs)
});
// checked pow will only be available with version 1.34, so for now, only perform operation
math!(
    Pow,
    pow,
    |lhs: f64, rhs: f64| { lhs.powf(rhs) },
    |lhs: i128, rhs: i128| { Some(lhs.pow(rhs as u32)) },
    true
);
