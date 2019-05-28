pub mod assignments;
mod loops;
pub mod pipelines;
mod quotes;
mod shell_expand;
mod statement;

pub use self::{
    loops::ForValueExpression,
    quotes::Terminator,
    shell_expand::{Expander, Select},
    statement::{is_valid_name, parse_and_validate, StatementSplitter},
};

#[cfg(fuzzing)]
pub mod fuzzing {
    use super::*;

    pub fn statement_parse(data: &str) { statement::parse::parse(data); }
}
