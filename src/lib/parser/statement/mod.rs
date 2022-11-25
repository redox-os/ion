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
use std::char;
use thiserror::Error;

pub type Result = std::result::Result<Statement, Error>;

/// An Error occured during parsing
#[derive(Debug, Error, PartialEq, Eq, Hash, Clone)]
pub enum Error {
    /// The command name is illegal
    #[error("illegal command name: {0}")]
    IllegalCommandName(String),
    /// Invalid character found
    #[error("syntax error: '{0}' at position {1} is out of place")]
    InvalidCharacter(char, usize),
    /// Unterminated subshell
    #[error("syntax error: unterminated subshell")]
    UnterminatedSubshell,
    /// Unterminated namespaced variable
    #[error("syntax error: unterminated brace var")]
    UnterminatedBracedVar,
    /// Unterminated square bracket
    #[error("syntax error: unterminated square bracket")]
    UnterminatedSquareBracket,
    /// Unterminated single quotes
    #[error("syntax error: unterminated single quotes")]
    UnterminatedSingleQuotes,
    /// Unterminated double quotes 
    #[error("syntax error: unterminated double quotes")]
    UnterminatedDoubleQuotes,
    /// Unterminated brace expansion
    #[error("syntax error: unterminated braced")]
    UnterminatedBrace,
    /// Unterminated method
    #[error("syntax error: unterminated method")]
    UnterminatedMethod,
    /// Unterminated arithmetic expression
    #[error("syntax error: unterminated arithmetic subexpression")]
    UnterminatedArithmetic,
    /// Expected command but found ...
    #[error("expected command, but found {0}")]
    ExpectedCommandButFound(&'static str),
    /// A match/case/for block lacked matching helpers
    #[error("missing parameters for a block")]
    IncompleteFlowControl,
    /// No keys were supplied for assignment
    #[error("no key supplied for assignment")]
    NoKeySupplied,
    /// No operator was supplied for assignment
    #[error("no operator supplied for assignment")]
    NoOperatorSupplied,
    /// No value supplied for assignment
    #[error("no values supplied for assignment")]
    NoValueSupplied,
    /// No value given for iteration in a for loop
    #[error("no value supplied for iteration in for loop")]
    NoInKeyword,
    /// Error with match statements
    #[error("case error: {0}")]
    Case(#[source] CaseError),
    /// The provided function name was invalid
    #[error(
        "'{0}' is not a valid function name
        Function names may only contain alphanumeric characters"
    )]
    InvalidFunctionName(String),
    /// The arguments did not match the function's signature
    #[error("function argument error: {0}")]
    InvalidFunctionArgument(#[source] FunctionParseError),
    /// Error occured during parsing of a pipeline
    #[error("{0}")]
    Pipeline(#[source] PipelineParsingError),
}

impl From<FunctionParseError> for Error {
    fn from(cause: FunctionParseError) -> Self { Self::InvalidFunctionArgument(cause) }
}

impl From<CaseError> for Error {
    fn from(cause: CaseError) -> Self { Self::Case(cause) }
}

impl From<PipelineParsingError> for Error {
    fn from(cause: PipelineParsingError) -> Self { Self::Pipeline(cause) }
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
