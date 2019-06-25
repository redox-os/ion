mod math;
mod modification;
pub mod types;

pub use self::{
    math::{EuclDiv, OpError, Pow},
    modification::Modifications,
};
use itertools::Itertools;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Value<T> {
    Str(types::Str),
    Alias(types::Alias),
    Array(types::Array<T>),
    HashMap(types::HashMap<T>),
    BTreeMap(types::BTreeMap<T>),
    Function(T),
    None,
}

impl<T: Eq> Eq for Value<T> {}

// this one’s only special because of the lifetime parameter
impl<'a, T> From<&'a str> for Value<T> {
    fn from(string: &'a str) -> Self {
        Value::Str(string.into())
    }
}

macro_rules! value_from_type {
    ($arg:ident: $from:ty => $variant:ident($inner:expr)) => {
        impl<T> From<$from> for Value<T> {
            fn from($arg: $from) -> Self {
                Value::$variant($inner)
            }
        }
    };
}

value_from_type!(string: types::Str => Str(string));
value_from_type!(string: String => Str(string.into()));
value_from_type!(alias: types::Alias => Alias(alias));
value_from_type!(array: types::Array<T> => Array(array));
value_from_type!(hmap: types::HashMap<T> => HashMap(hmap));
value_from_type!(bmap: types::BTreeMap<T> => BTreeMap(bmap));

impl<T> fmt::Display for Value<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Value::Str(ref str_) => write!(f, "{}", str_),
            Value::Alias(ref alias) => write!(f, "{}", **alias),
            Value::Array(ref array) => write!(f, "{}", array.iter().format(" ")),
            Value::HashMap(ref map) => write!(f, "{}", map.values().format(" ")),
            Value::BTreeMap(ref map) => write!(f, "{}", map.values().format(" ")),
            _ => write!(f, ""),
        }
    }
}

#[cfg(test)]
mod trait_test;
