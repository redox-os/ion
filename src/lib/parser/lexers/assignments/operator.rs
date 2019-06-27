use std::fmt::{self, Display, Formatter};

/// An operation to do on a value
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operator {
    /// Addition (only works on numeric types)
    Add,
    /// Concatenation (will also concat numeric types)
    Concatenate,
    /// Prepend the value (will also concat numeric types)
    ConcatenateHead,
    /// Division (only works on numeric types)
    Divide,
    /// Assignment
    Equal,
    /// Assign a default value
    OptionalEqual,
    /// Exponent (only works on numeric types)
    Exponent,
    /// Filter the array to remove the matching values (only works on array and map-like types)
    Filter,
    /// Euclidian Division (only available on numeric types, and works on floats too)
    IntegerDivide,
    /// Muliplication (only works on numeric types)
    Multiply,
    /// Substraction (only works on numeric types)
    Subtract,
}

impl Operator {
    pub(crate) fn parse_single(data: u8) -> Option<Self> {
        match data {
            b'+' => Some(Operator::Add),
            b'-' => Some(Operator::Subtract),
            b'/' => Some(Operator::Divide),
            b'*' => Some(Operator::Multiply),
            b'?' => Some(Operator::OptionalEqual),
            _ => None,
        }
    }

    pub(crate) fn parse_double(data: &[u8]) -> Option<Self> {
        match data {
            b"//" => Some(Operator::IntegerDivide),
            b"**" => Some(Operator::Exponent),
            b"++" => Some(Operator::Concatenate),
            b"::" => Some(Operator::ConcatenateHead),
            b"\\\\" => Some(Operator::Filter),
            _ => None,
        }
    }
}

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Operator::Add => "+=",
                Operator::Concatenate => "++=",
                Operator::ConcatenateHead => "::=",
                Operator::Filter => "\\\\=",
                Operator::Divide => "/=",
                Operator::Equal => "=",
                Operator::OptionalEqual => "?=",
                Operator::Exponent => "**=",
                Operator::IntegerDivide => "//=",
                Operator::Multiply => "*=",
                Operator::Subtract => "-=",
            }
        )
    }
}
