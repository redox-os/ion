mod arrays;
mod strings;

use self::strings::unescape;
pub use self::{arrays::ArrayMethod, strings::StringMethod};

use super::Expander;
use crate::{lexers::ArgumentSplitter, types};
use err_derive::Error;

#[derive(Debug, PartialEq, Clone)]
pub enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}

pub type Result<T> = std::result::Result<T, MethodError>;

#[derive(Debug)]
pub struct MethodArgs<'a, 'b, E: Expander> {
    args:   &'a str,
    expand: &'b E,
}

#[derive(Debug, Clone, Error)]
pub enum MethodError {
    #[error(display = "'{}' is an unknown array method", _0)]
    InvalidArrayMethod(String),
    #[error(display = "'{}' is an unknown string method", _0)]
    InvalidScalarMethod(String),
    #[error(display = "{}: {}", _0, _1)]
    WrongArgument(&'static str, &'static str),

    // specific to some builtins
    #[error(display = "regex_replace: error in regular expression '{}': {}", _0, _1)]
    InvalidRegex(String, #[error(cause)] regex::Error),
    #[error(display = "{}: no map found", _0)]
    NoMapFound(&'static str),
    #[error(display = "split_at: value is out of bounds")]
    OutOfBound,
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub fn array<'c>(&'c self) -> impl Iterator<Item = types::Str> + 'c {
        ArgumentSplitter::new(self.args)
            .flat_map(move |x| self.expand.expand_string(x))
            .map(|s| unescape(&s))
    }

    pub fn join(self, pattern: &str) -> types::Str {
        unescape(&self.expand.expand_string(self.args).join(pattern))
    }

    pub fn new(args: &'a str, expand: &'b E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand }
    }
}
