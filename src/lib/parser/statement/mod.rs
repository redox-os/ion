mod case;
mod functions;
mod parse;
mod splitter;

pub use self::{
    parse::{is_valid_name, parse},
    splitter::{StatementSplitter, StatementVariant},
};
use super::{
    pipelines::PipelineParsingError,
    statement::{case::Error as CaseError, functions::FunctionParseError},
};
use crate::{builtins::BuiltinMap, shell::flow_control::Statement};
use err_derive::Error;
use std::char;

pub type Result<'a> = std::result::Result<Statement<'a>, ParseError>;

/// An Error occured during parsing
#[derive(Debug, Error, PartialEq, Eq, Hash, Clone)]
pub enum ParseError {
    /// The command name is illegal
    #[error(display = "illegal command name: {}", _0)]
    IllegalCommandName(String),
    /// Invalid character found
    #[error(display = "syntax error: '{}' at position {} is out of place", _0, _1)]
    InvalidCharacter(char, usize),
    /// Unterminated subshell
    #[error(display = "syntax error: unterminated subshell")]
    UnterminatedSubshell,
    /// Unterminated namespaced variable
    #[error(display = "syntax error: unterminated brace")]
    UnterminatedBracedVar,
    /// Unterminated brace expansion
    #[error(display = "syntax error: unterminated braced var")]
    UnterminatedBrace,
    /// Unterminated method
    #[error(display = "syntax error: unterminated method")]
    UnterminatedMethod,
    /// Unterminated arithmetic expression
    #[error(display = "syntax error: unterminated arithmetic subexpression")]
    UnterminatedArithmetic,
    /// Expected command but found ...
    #[error(display = "expected command, but found {}", _0)]
    ExpectedCommandButFound(&'static str),
    /// A match/case/for block lacked matching helpers
    #[error(display = "missing parameters for a block")]
    IncompleteFlowControl,
    /// No keys were supplied for assignment
    #[error(display = "no key supplied for assignment")]
    NoKeySupplied,
    /// No operator was supplied for assignment
    #[error(display = "no operator supplied for assignment")]
    NoOperatorSupplied,
    /// No value supplied for assignment
    #[error(display = "no values supplied for assignment")]
    NoValueSupplied,
    /// No value given for iteration in a for loop
    #[error(display = "no value supplied for iteration in for loop")]
    NoInKeyword,
    /// Error with match statements
    #[error(display = "case error: {}", _0)]
    CaseError(#[error(cause)] CaseError),
    /// The provided function name was invalid
    #[error(
        display = "'{}' is not a valid function name
        Function names may only contain alphanumeric characters",
        _0
    )]
    InvalidFunctionName(String),
    /// The arguments did not match the function's signature
    #[error(display = "function argument error: {}", _0)]
    InvalidFunctionArgument(#[error(cause)] FunctionParseError),
    /// Error occured during parsing of a pipeline
    #[error(display = "{}", _0)]
    PipelineParsingError(#[error(cause)] PipelineParsingError),
}

impl From<FunctionParseError> for ParseError {
    fn from(cause: FunctionParseError) -> Self { ParseError::InvalidFunctionArgument(cause) }
}

impl From<CaseError> for ParseError {
    fn from(cause: CaseError) -> Self { ParseError::CaseError(cause) }
}

impl From<PipelineParsingError> for ParseError {
    fn from(cause: PipelineParsingError) -> Self { ParseError::PipelineParsingError(cause) }
}

/// Parses a given statement string and return's the corresponding mapped
/// `Statement`
pub fn parse_and_validate<'b>(
    statement: StatementVariant<'_>,
    builtins: &BuiltinMap<'b>,
) -> Result<'b> {
    match statement {
        StatementVariant::And(statement) => {
            Ok(Statement::And(Box::new(parse(statement, builtins)?)))
        }
        StatementVariant::Or(statement) => Ok(Statement::Or(Box::new(parse(statement, builtins)?))),
        StatementVariant::Default(statement) => parse(statement, builtins),
    }
}

/// Splits a string into two, based on a given pattern. We know that the first string will always
/// exist, but if the pattern is not found, or no string follows the pattern, then the second
/// string will not exist. Useful for splitting the function expression by the "--" pattern.
fn split_pattern<'a>(arg: &'a str, pattern: &str) -> (&'a str, Option<&'a str>) {
    match arg.find(pattern) {
        Some(pos) => {
            let args = &arg[..pos].trim();
            let comment = &arg[pos + pattern.len()..].trim();
            if comment.is_empty() {
                (args, None)
            } else {
                (args, Some(comment))
            }
        }
        None => (arg, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_pattern_splitting() {
        let (args, description) = split_pattern("a:int b:bool -- a comment", "--");
        assert_eq!(args, "a:int b:bool");
        assert_eq!(description, Some("a comment"));

        let (args, description) = split_pattern("a --", "--");
        assert_eq!(args, "a");
        assert_eq!(description, None);

        let (args, description) = split_pattern("a", "--");
        assert_eq!(args, "a");
        assert_eq!(description, None);
    }
}
