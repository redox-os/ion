mod arrays;
mod strings;

use self::strings::unescape;
pub(crate) use self::{arrays::ArrayMethod, strings::StringMethod};

use super::{expand_string, Expander};
use crate::lexers::ArgumentSplitter;
use small;

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}

#[derive(Debug)]
pub(crate) struct MethodArgs<'a, 'b, E: 'b + Expander> {
    args:   &'a str,
    expand: &'b E,
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub(crate) fn array<'c>(&'c self) -> impl Iterator<Item = small::String> + 'c {
        ArgumentSplitter::new(self.args)
            .flat_map(move |x| expand_string(x, self.expand, false).into_iter())
            .map(|s| unescape(&s).unwrap_or_default())
    }

    pub(crate) fn join(self, pattern: &str) -> small::String {
        unescape(&expand_string(self.args, self.expand, false).join(pattern)).unwrap_or_default()
    }

    pub(crate) fn new(args: &'a str, expand: &'b E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand }
    }
}
