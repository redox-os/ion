mod arguments;
pub(crate) mod assignments;
mod loops;
pub(crate) mod pipelines;
mod quotes;
pub(crate) mod shell_expand;
mod statement;

pub use self::{arguments::ArgumentSplitter, assignments::Primitive, quotes::Terminator};
pub(crate) use self::{
    loops::for_grammar::ForExpression,
    shell_expand::{expand_string, Expander, Select},
    statement::{parse_and_validate, StatementSplitter},
};
