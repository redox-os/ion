mod arrays;
mod strings;

use self::strings::unescape;
pub use self::{arrays::ArrayMethod, strings::StringMethod};

use super::Expander;
use crate::{parser::lexers::ArgumentSplitter, types};
use err_derive::Error;

#[derive(Debug, PartialEq, Clone)]
pub enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}

#[derive(Debug)]
pub struct MethodArgs<'a, 'b, E: Expander> {
    args:   &'a str,
    expand: &'b mut E,
}

/// Error during method expansion
///
/// Ex: `$join($scalar)` (can't join a scala) or `$unknown(@variable)` (unknown method)
#[derive(Debug, Clone, Error)]
pub enum MethodError {
    /// Unknown array method
    #[error(display = "'{}' is an unknown array method", _0)]
    InvalidArrayMethod(String),
    /// Unknown scalar method
    #[error(display = "'{}' is an unknown string method", _0)]
    InvalidScalarMethod(String),
    /// A wrong argumeng was given to the method (extra, missing, or wrong type)
    #[error(display = "{}: {}", _0, _1)]
    WrongArgument(&'static str, &'static str),

    /// An invalid regex was provided. This is specific to the `matches` method
    #[error(display = "regex_replace: error in regular expression '{}': {}", _0, _1)]
    InvalidRegex(String, #[error(source)] regex::Error),
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub fn array<'c>(&'c mut self) -> impl Iterator<Item = types::Str> + 'c {
        let expand = &mut (*self.expand);
        ArgumentSplitter::new(self.args)
            .flat_map(move |x| expand.expand_string(x).unwrap_or_else(|_| types::Args::new()))
            .map(|s| unescape(&s))
    }

    pub fn join(self, pattern: &str) -> super::Result<types::Str, E::Error> {
        Ok(unescape(&self.expand.expand_string(self.args)?.join(pattern)))
    }

    pub fn new(args: &'a str, expand: &'b mut E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand }
    }
}
