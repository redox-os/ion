use small;
use smallvec::SmallVec;
pub use types_rs::types::*;

pub use crate::shell::flow_control::Function;
pub type Args = SmallVec<[small::String; 4]>;
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
macro_rules! args [
    ( $($x:expr), *) => ({
        let mut _arr = crate::types::Args::new();
        $(_arr.push($x.into());)*
        _arr
    })
];
