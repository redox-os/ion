mod arrays;
mod pattern;
mod strings;

pub(crate) use self::arrays::ArrayMethod;
pub(crate) use self::pattern::Pattern;
pub(crate) use self::strings::StringMethod;

use super::{Index, Range, Expander, expand_string};
use super::super::ranges::parse_index_range;
use super::super::super::ArgumentSplitter;
use std::iter::{empty, FromIterator};
use std::str::FromStr;
use self::pattern::unescape;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Key {
    key: ::types::Key,
}

impl Key {
    #[cfg(test)]
    pub(crate) fn new<K: Into<::types::Key>>(key: K) -> Key { Key { key: key.into() } }
    pub(crate) fn get(&self) -> &::types::Key { return &self.key; }
}

/// Represents a filter on a vector-like object
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Select {
    /// Select no elements
    None,
    /// Select all elements
    All,
    /// Select a single element based on its index
    Index(Index),
    /// Select a range of elements
    Range(Range),
    /// Select an element by mapped key
    Key(Key),
}

pub(crate) trait SelectWithSize {
    type Item;
    fn select<O>(&mut self, Select, usize) -> O
        where O: FromIterator<Self::Item>;
}

impl<I, T> SelectWithSize for I
    where I: Iterator<Item = T>
{
    type Item = T;
    fn select<O>(&mut self, s: Select, size: usize) -> O
        where O: FromIterator<Self::Item>
    {
        match s {
            Select::None => empty().collect(),
            Select::All => self.collect(),
            Select::Index(idx) => {
                idx.resolve(size).and_then(|idx| self.nth(idx)).into_iter().collect()
            }
            Select::Range(range) => if let Some((start, length)) = range.bounds(size) {
                self.skip(start).take(length).collect()
            } else {
                empty().collect()
            },
            Select::Key(_) => empty().collect(),
        }
    }
}

impl FromStr for Select {
    type Err = ();
    fn from_str(data: &str) -> Result<Select, ()> {
        if ".." == data {
            return Ok(Select::All);
        }

        if let Ok(index) = data.parse::<isize>() {
            return Ok(Select::Index(Index::new(index)));
        }

        if let Some(range) = parse_index_range(data) {
            return Ok(Select::Range(range));
        }

        Ok(Select::Key(Key { key: data.into() }))
    }
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