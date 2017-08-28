mod functions;
mod parse;
mod splitter;

pub use self::parse::parse;
pub use self::splitter::{StatementError, StatementSplitter};
use shell::flow_control::Statement;

/// Parses a given statement string and return's the corresponding mapped `Statement`
pub fn parse_and_validate<'a>(statement: Result<&str, StatementError<'a>>) -> Statement {
    match statement {
        Ok(statement) => parse(statement),
        Err(err) => {
            eprintln!("ion: {}", err);
            Statement::Error(-1)
        }
    }
}

/// Splits a string into two, based on a given pattern. We know that the first string will always
/// exist, but if the pattern is not found, or no string follows the pattern, then the second
/// string will not exist. Useful for splitting the function expression by the "--" pattern.
pub fn split_pattern<'a>(arg: &'a str, pattern: &str) -> (&'a str, Option<&'a str>) {
    match arg.find(pattern) {
        Some(pos) => {
            let args = &arg[..pos].trim();
            let comment = &arg[pos + pattern.len()..].trim();
            if comment.is_empty() { (args, None) } else { (args, Some(comment)) }
        }
        None => (arg, None),
    }
}
