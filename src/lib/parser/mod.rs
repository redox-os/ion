pub(crate) mod assignments;
mod loops;
pub(crate) mod pipelines;
mod quotes;
pub(crate) mod shell_expand;
mod statement;

pub use self::quotes::Terminator;
pub(crate) use self::{
    loops::ForExpression, shell_expand::{expand_string, Expander, Select},
    statement::{parse_and_validate, StatementSplitter},
};

#[cfg(fuzzing)]
pub mod fuzzing {
    use super::*;

    pub fn statement_parse(data: &str) {
        statement::parse::parse(data);
    }
}
