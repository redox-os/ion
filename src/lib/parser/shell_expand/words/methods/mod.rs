mod arrays;
mod strings;

pub(crate) use self::{arrays::ArrayMethod, strings::StringMethod};

use self::strings::unescape;
use super::{super::super::ArgumentSplitter, expand_string, Expander};

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Key {
    pub(crate) key: ::types::Key,
}

impl Key {
    pub(crate) fn get(&self) -> &::types::Key { return &self.key; }

    #[cfg(test)]
    pub(crate) fn new<K: Into<::types::Key>>(key: K) -> Key { Key { key: key.into() } }
}

#[derive(Debug)]
pub(crate) struct MethodArgs<'a, 'b, E: 'b + Expander> {
    args:   &'a str,
    expand: &'b E,
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub(crate) fn array<'c>(&'c self) -> impl Iterator<Item = String> + 'c {
        ArgumentSplitter::new(self.args)
            .flat_map(move |x| expand_string(x, self.expand, false).into_iter())
            .map(|s| unescape(&s).unwrap_or(String::from("")))
    }

    pub(crate) fn join(self, pattern: &str) -> String {
        unescape(&expand_string(self.args, self.expand, false).join(pattern))
            .unwrap_or(String::from(""))
    }

    pub(crate) fn new(args: &'a str, expand: &'b E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand }
    }
}
