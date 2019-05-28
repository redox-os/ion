pub(crate) mod assignments;
mod loops;
pub(crate) mod pipelines;
mod quotes;
pub(crate) mod shell_expand;
mod statement;

pub(crate) use self::{
    loops::ForValueExpression,
    shell_expand::{expand_string, Select},
};
pub use self::{
    quotes::Terminator,
    shell_expand::Expander,
    statement::{is_valid_name, parse_and_validate, StatementSplitter},
};

#[cfg(fuzzing)]
pub mod fuzzing {
    use super::*;

    pub fn statement_parse(data: &str) { statement::parse::parse(data); }
}
