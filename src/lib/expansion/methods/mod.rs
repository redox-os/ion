mod arrays;
mod strings;

use self::strings::unescape;
pub use self::{arrays::ArrayMethod, strings::StringMethod};

use super::Expander;
use crate::{parser::lexers::ArgumentSplitter, types};
use thiserror::Error;

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
    #[error("'{0}' is an unknown array method")]
    InvalidArrayMethod(String),
    /// Unknown scalar method
    #[error("'{0}' is an unknown string method")]
    InvalidScalarMethod(String),
    /// A wrong argumeng was given to the method (extra, missing, or wrong type)
    #[error("{0}: {1}")]
    WrongArgument(&'static str, &'static str),

    /// An invalid regex was provided. This is specific to the `matches` method
    #[error("regex_replace: error in regular expression '{0}': {1}")]
    InvalidRegex(String, #[source] regex::Error),
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub fn array(&mut self) -> impl Iterator<Item = types::Str> + '_ {
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
