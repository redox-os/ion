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
    args:                       &'a str,
    expand:                     &'b mut E,
    /// If true then the third argument may be an empty string.
    /// Currently used for method replace, replacen and regex_replace
    /// Need to use this ad hoc approach because several other integration tests
    /// fail if empty string arguments are allowed always.
    is_empty_third_arg_allowed: bool,
}

impl<'a, 'b, E: Expander> MethodArgs<'a, 'b, E> {
    pub fn allow_third_args_empty(&mut self) { self.is_empty_third_arg_allowed = true; }
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
    /// A wrong argument was given to the method (extra, missing, or wrong type)
    #[error("{0}: {1}")]
    WrongArgument(&'static str, &'static str),

    /// An invalid regex was provided. This is specific to the `matches` method
    #[error("regex_replace: error in regular expression '{0}': {1}")]
    InvalidRegex(String, #[source] regex::Error),
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub fn array(&mut self) -> impl Iterator<Item = types::Str> + '_ {
        let expand = &mut (*self.expand);
        let allow_for_empty_args = self.is_empty_third_arg_allowed;
        ArgumentSplitter::new(self.args)
            .enumerate()
            .flat_map(move |(index, next_args)| {
                expand
                    .expand_string(next_args)
                    .map(move |might_be_empty| {
                        // If an argument is an empty string like "" or '' then
                        // expand_string returns an empty Args vec.
                        // Flat map would remove this empty string argument.
                        // If a string methods allows a third empty string arg like replace
                        // that then if invention is needed.
                        if allow_for_empty_args && index == 1 && might_be_empty.is_empty() {
                            // index == 1 corresponds to the third argument in a string method
                            // $replace("input" "to_replace" "")
                            //                               ^ third argument
                            args![""]
                        } else {
                            might_be_empty
                        }
                    })
                    .unwrap_or_else(|_| types::Args::new())
            })
            .map(|s| unescape(&s))
    }

    pub fn join(self, pattern: &str) -> super::Result<types::Str, E::Error> {
        Ok(unescape(&self.expand.expand_string(self.args)?.join(pattern)))
    }

    pub fn new(args: &'a str, expand: &'b mut E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand, is_empty_third_arg_allowed: false }
    }
}
