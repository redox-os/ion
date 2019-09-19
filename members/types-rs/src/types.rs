use super::Value;
use small;
use std::{
    collections::{BTreeMap as StdBTreeMap, HashMap as StdHashMap},
    iter::FromIterator,
    ops::{Deref, DerefMut},
};

pub type Array<T> = Vec<Value<T>>;
pub type HashMap<T> = StdHashMap<Str, Value<T>>;
pub type BTreeMap<T> = StdBTreeMap<Str, Value<T>>;
pub type Str = small::String;

#[derive(Clone, Debug, PartialEq, Hash, Eq, Default)]
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

impl<T> FromIterator<Value<T>> for Value<T> {
    fn from_iter<I: IntoIterator<Item = Value<T>>>(items: I) -> Self {
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
        let mut _arr = $crate::types::Array::new();
        $(_arr.push($x.into());)*
        _arr
    })
];
