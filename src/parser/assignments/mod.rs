mod actions;
mod checker;
mod splitter;
mod keys;
mod operator;

pub(crate) use self::actions::{Action, AssignmentActions, AssignmentError};
pub(crate) use self::checker::{is_array, value_check};
pub(crate) use self::keys::{Key, KeyBuf, KeyIterator, Primitive, TypeError};
pub(crate) use self::operator::Operator;
pub(crate) use self::splitter::split_assignment;

use types::{Array, Value};

#[derive(Debug, PartialEq)]
pub(crate) enum ReturnValue {
    Str(Value),
    Vector(Array),
}
