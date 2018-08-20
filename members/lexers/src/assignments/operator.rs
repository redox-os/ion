use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operator {
    Add,
    Concatenate,
    ConcatenateHead,
    Divide,
    Equal,
    OptionalEqual,
    Exponent,
    Filter,
    IntegerDivide,
    Multiply,
    Subtract,
}

impl Operator {
    pub(crate) fn parse_single(data: u8) -> Option<Operator> {
        match data {
            b'+' => Some(Operator::Add),
            b'-' => Some(Operator::Subtract),
            b'/' => Some(Operator::Divide),
            b'*' => Some(Operator::Multiply),
            b'?' => Some(Operator::OptionalEqual),
            _ => None,
        }
    }

    pub(crate) fn parse_double(data: &[u8]) -> Option<Operator> {
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
