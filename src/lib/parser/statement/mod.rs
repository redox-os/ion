mod case;
mod functions;
mod parse;
mod splitter;

pub use self::{
    parse::{is_valid_name, parse, ParseError},
    splitter::{StatementError, StatementSplitter, StatementVariant},
};
use crate::{builtins::BuiltinMap, shell::flow_control::Statement};

pub type Result<'a> = std::result::Result<Statement<'a>, ParseError>;

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
