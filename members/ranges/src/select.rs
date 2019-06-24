use super::{parse_index_range, Index, Range};
use std::{
    iter::{empty, FromIterator},
    str::FromStr,
};

/// Represents a filter on a vector-like object
#[derive(Debug, PartialEq, Clone)]
pub enum Select<K> {
    /// Select all elements
    All,
    /// Select a single element based on its index
    Index(Index),
    /// Select a range of elements
    Range(Range),
    /// Select an element by mapped key
    Key(K),
}

pub trait SelectWithSize {
    type Item;
    fn select<O, K>(&mut self, selection: &Select<K>, len: usize) -> O
    where
        O: FromIterator<Self::Item>;
}

impl<I, T> SelectWithSize for I
where
    I: DoubleEndedIterator<Item = T>,
{
    type Item = T;

    fn select<O, K>(&mut self, s: &Select<K>, size: usize) -> O
    where
        O: FromIterator<Self::Item>,
    {
        match s {
            Select::Key(_) => empty().collect(),
            Select::All => self.collect(),
            Select::Index(Index::Forward(idx)) => self.nth(*idx).into_iter().collect(),
            Select::Index(Index::Backward(idx)) => self.rev().nth(*idx).into_iter().collect(),
            Select::Range(range) => range
                .bounds(size)
                .map(|(start, length)| self.skip(start).take(length).collect())
                .unwrap_or_else(|| empty().collect()),
        }
    }
}

impl<K: FromStr> FromStr for Select<K> {
    type Err = ();

    fn from_str(data: &str) -> Result<Self, ()> {
        if data == ".." {
            Ok(Select::All)
        } else if let Ok(index) = data.parse::<isize>() {
            Ok(Select::Index(Index::new(index)))
        } else if let Some(range) = parse_index_range(data) {
            Ok(Select::Range(range))
        } else {
            Ok(Select::Key(K::from_str(data).map_err(|_| ())?))
        }
    }
}
