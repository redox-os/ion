mod arguments;
pub(crate) mod assignments;
mod loops;
pub(crate) mod pipelines;
pub(crate) mod shell_expand;
pub(crate) mod variables;
mod statement;
mod quotes;

pub use self::arguments::ArgumentSplitter;
pub use self::assignments::Primitive;
pub(crate) use self::loops::for_grammar::ForExpression;
pub use self::quotes::Terminator;
pub(crate) use self::shell_expand::{expand_string, Expander, Select};
pub(crate) use self::statement::{parse_and_validate, StatementSplitter};
