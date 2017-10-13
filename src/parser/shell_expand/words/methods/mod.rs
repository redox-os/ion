mod arrays;
mod pattern;
mod strings;

pub(crate) use self::arrays::ArrayMethod;
pub(crate) use self::pattern::Pattern;
pub(crate) use self::strings::StringMethod;

use super::{Expander, expand_string};
use super::super::super::ArgumentSplitter;
use self::pattern::unescape;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Key {
    pub(crate) key: ::types::Key,
}

impl Key {
    #[cfg(test)]
    pub(crate) fn new<K: Into<::types::Key>>(key: K) -> Key { Key { key: key.into() } }
    pub(crate) fn get(&self) -> &::types::Key { return &self.key; }
}

pub(crate) struct MethodArgs<'a, 'b, E: 'b + Expander> {
    args:   &'a str,
    expand: &'b E,
}

impl<'a, 'b, E: 'b + Expander> MethodArgs<'a, 'b, E> {
    pub(crate) fn new(args: &'a str, expand: &'b E) -> MethodArgs<'a, 'b, E> {
        MethodArgs { args, expand }
    }

    pub(crate) fn join(self, pattern: &str) -> String {
        unescape(expand_string(self.args, self.expand, false).join(pattern))
    }

    pub(crate) fn array<'c>(&'c self) -> impl Iterator<Item = String> + 'c {
        ArgumentSplitter::new(self.args)
            .flat_map(move |x| expand_string(x, self.expand, false).into_iter())
            .map(unescape)
    }
}