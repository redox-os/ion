mod case;
mod functions;
mod parse;
mod splitter;

pub use self::{
    parse::parse,
    splitter::{StatementSplitter, StatementVariant},
};
use super::{
    pipelines::PipelineParsingError,
    statement::{case::Error as CaseError, functions::FunctionParseError},
};
use crate::{builtins::BuiltinMap, shell::flow_control::Statement};
use err_derive::Error;
use std::char;

pub type Result = std::result::Result<Statement, Error>;

/// An Error occured during parsing
#[derive(Debug, Error, PartialEq, Eq, Hash, Clone)]
#[error(no_from)]
pub enum Error {
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
    Case(#[error(source)] CaseError),
    /// The provided function name was invalid
    #[error(
        display = "'{}' is not a valid function name
        Function names may only contain alphanumeric characters",
        _0
    )]
    InvalidFunctionName(String),
    /// The arguments did not match the function's signature
    #[error(display = "function argument error: {}", _0)]
    InvalidFunctionArgument(#[error(source)] FunctionParseError),
    /// Error occured during parsing of a pipeline
    #[error(display = "{}", _0)]
    Pipeline(#[error(source)] PipelineParsingError),
}

impl From<FunctionParseError> for Error {
    fn from(cause: FunctionParseError) -> Self { Error::InvalidFunctionArgument(cause) }
}

impl From<CaseError> for Error {
    fn from(cause: CaseError) -> Self { Error::Case(cause) }
}

impl From<PipelineParsingError> for Error {
    fn from(cause: PipelineParsingError) -> Self { Error::Pipeline(cause) }
}

/// Parses a given statement string and return's the corresponding mapped
/// `Statement`
pub fn parse_and_validate<'b>(statement: StatementVariant, builtins: &BuiltinMap<'b>) -> Result {
    match statement {
        StatementVariant::And(statement) => {
            Ok(Statement::And(Box::new(parse(statement, builtins)?)))
        }
        StatementVariant::Or(statement) => Ok(Statement::Or(Box::new(parse(statement, builtins)?))),
        StatementVariant::Default(statement) => parse(statement, builtins),
    }
}
