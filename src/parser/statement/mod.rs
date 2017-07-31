mod parse;
mod splitter;

pub use self::parse::{get_function_args, parse};
pub use self::splitter::{StatementError, StatementSplitter};
use shell::flow_control::Statement;

pub fn parse_and_validate<'a>(statement: Result<&str, StatementError<'a>>) -> Statement {
    match statement {
        Ok(statement) => parse(statement),
        Err(err) => {
            eprintln!("ion: {}", err);
            Statement::Error(-1)
        }
    }
}
