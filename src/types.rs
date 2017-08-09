
use fnv::FnvHashMap;
use smallstring::SmallString;
use smallvec::SmallVec;

pub type Array = SmallVec<[Value; 4]>;
pub type HashMap = FnvHashMap<Key, Value>;
pub type Identifier = SmallString;
pub type Key = SmallString;
pub type Value = String;
pub type VariableContext = FnvHashMap<Identifier, Value>;
pub type ArrayVariableContext = FnvHashMap<Identifier, Array>;
pub type HashMapVariableContext = FnvHashMap<Identifier, HashMap>;

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
/// ```
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
