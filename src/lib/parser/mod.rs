pub mod pipelines;
mod quotes;
mod statement;

pub use self::{
    quotes::Terminator,
    statement::{is_valid_name, parse_and_validate, ParseError, StatementError, StatementSplitter},
};

#[cfg(fuzzing)]
pub mod fuzzing {
    use super::*;

    pub fn statement_parse(data: &str) { statement::parse::parse(data); }
}
