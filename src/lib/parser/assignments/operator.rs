use super::AssignmentError;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum Operator {
    Add,
    Subtract,
    Divide,
    IntegerDivide,
    Multiply,
    Exponent,
    Equal,
}

impl Operator {
    pub(crate) fn parse(data: &str) -> Result<Operator, AssignmentError> {
        match data {
            "=" => Ok(Operator::Equal),
            "+=" => Ok(Operator::Add),
            "-=" => Ok(Operator::Subtract),
            "/=" => Ok(Operator::Divide),
            "//=" => Ok(Operator::IntegerDivide),
            "*=" => Ok(Operator::Multiply),
            "**=" => Ok(Operator::Exponent),
            _ => Err(AssignmentError::InvalidOperator(data)),
        }
    }
}

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Operator::Add => write!(f, "+="),
            Operator::Subtract => write!(f, "-="),
            Operator::Divide => write!(f, "/="),
            Operator::IntegerDivide => write!(f, "//="),
            Operator::Multiply => write!(f, "*="),
            Operator::Exponent => write!(f, "**="),
            Operator::Equal => write!(f, "="),
        }
    }
}
