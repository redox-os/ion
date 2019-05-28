mod arrays;
mod strings;

use self::strings::unescape;
pub use self::{arrays::ArrayMethod, strings::StringMethod};

use super::{expand_string, Expander};
use crate::lexers::ArgumentSplitter;
use small;

#[derive(Debug, PartialEq, Clone)]
pub enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}

#[derive(Debug)]
pub struct MethodArgs<'a, 'b, E: 'b + Expander> {
    args:   &'a str,
    expand: &'b E,
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub fn array<'c>(&'c self) -> impl Iterator<Item = small::String> + 'c {
        ArgumentSplitter::new(self.args)
            .flat_map(move |x| expand_string(x, self.expand).into_iter())
            .map(|s| unescape(&s).unwrap_or_default())
    }

    pub fn join(self, pattern: &str) -> small::String {
        unescape(&expand_string(self.args, self.expand).join(pattern)).unwrap_or_default()
    }

    pub fn new(args: &'a str, expand: &'b E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand }
    }
}
