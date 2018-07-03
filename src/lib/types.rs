use fnv::FnvHashMap;
use smallstring::SmallString;
use smallvec::SmallVec;
use shell::variables::VariableType;
use std::{collections::BTreeMap as StdBTreeMap, ops::{Deref, DerefMut}};

pub type Array = SmallVec<[Value; 4]>;
pub type HashMap = FnvHashMap<Key, VariableType>;
pub type BTreeMap = StdBTreeMap<String, VariableType>;
pub type Identifier = SmallString;
pub type Key = SmallString;
pub type Value = String;

#[derive(Clone, Debug, PartialEq)]
pub struct Alias(pub String);

impl Alias {
    pub fn empty() -> Self {
        Alias(String::with_capacity(0))
    }
}

impl Deref for Alias {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Alias {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Into<String> for Alias {
    fn into(self) -> String {
        self.0
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
/// let verbose = Array::from_vec(vec!["foo".into(), "bar".into(),
///                                    "baz".into(), "zar".into(),
///                                    "doz".into()]);
/// let concise = array!["foo", "bar", "baz", "zar", "doz"];
/// assert_eq!(verbose, concise);
/// ```
#[macro_export]
macro_rules! array [
    ( $($x:expr), *) => ({
        let mut _arr = Array::new();
        $(_arr.push($x.into());)*
        _arr
    })
];
