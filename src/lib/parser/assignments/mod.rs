mod actions;
mod checker;
mod keys;
mod operator;
mod splitter;

pub use self::keys::Primitive;
pub(crate) use self::{
    actions::{Action, AssignmentActions, AssignmentError}, checker::{is_array, value_check},
    keys::{Key, KeyBuf, KeyIterator, TypeError}, operator::Operator, splitter::split_assignment,
};
