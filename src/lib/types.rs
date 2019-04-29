use crate::shell::variables::Value;
use hashbrown::HashMap as HashbrownMap;
use small;
use smallvec::SmallVec;
use std::{
    collections::BTreeMap as StdBTreeMap,
    iter::FromIterator,
    ops::{Deref, DerefMut},
};

pub type Args = SmallVec<[small::String; 4]>;
pub type Array<'a> = Vec<Value<'a>>;
pub type HashMap<'a> = HashbrownMap<Str, Value<'a>>;
pub type BTreeMap<'a> = StdBTreeMap<Str, Value<'a>>;
pub type Str = small::String;

#[derive(Clone, Debug, PartialEq)]
pub struct Alias(pub Str);

impl Alias {
    pub fn empty() -> Self { Alias(Str::with_capacity(1)) }
}

impl Deref for Alias {
    type Target = Str;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for Alias {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl Into<Str> for Alias {
    fn into(self) -> Str { self.0 }
}

impl<'a> FromIterator<Value<'a>> for Value<'a> {
    fn from_iter<I: IntoIterator<Item = Value<'a>>>(items: I) -> Self {
        Value::Array(items.into_iter().collect())
    }
}

/// Construct a new Array containing the given arguments
///
/// `array!` acts like the standard library's `vec!` macro, and can be thought
/// of as a shorthand for:
/// ```ignore,rust
/// Array::from_vec(vec![...])
/// ```
/// Additionally it will call `Into::into` on each of its members so that one
/// can pass in any type with some `To<SmallString>` implementation; they will
/// automatically be converted to owned SmallStrings.
/// ```ignore,rust
/// let verbose = Array::from_vec(vec![
///     "foo".into(),
///     "bar".into(),
///     "baz".into(),
///     "zar".into(),
///     "doz".into(),
/// ]);
/// let concise = array!["foo", "bar", "baz", "zar", "doz"];
/// assert_eq!(verbose, concise);
/// ```
#[macro_export]
macro_rules! array [
    ( $($x:expr), *) => ({
        let mut _arr = crate::types::Array::new();
        $(_arr.push($x.into());)*
        _arr
    })
];

#[macro_export]
macro_rules! args [
    ( $($x:expr), *) => ({
        let mut _arr = crate::types::Args::new();
        $(_arr.push($x.into());)*
        _arr
    })
];
