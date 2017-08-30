mod actions;
mod checker;
mod splitter;
mod keys;
mod operator;

pub use self::actions::{Action, AssignmentActions, AssignmentError};
pub use self::checker::{is_array, is_boolean, value_check};
pub use self::keys::{Key, KeyBuf, KeyIterator, Primitive, TypeError};
pub use self::operator::Operator;
pub use self::splitter::split_assignment;

use types::{Array, Value};

#[derive(Debug, PartialEq)]
pub enum ReturnValue {
    Str(Value),
    Vector(Array),
}
