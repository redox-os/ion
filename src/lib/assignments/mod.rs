mod actions;
mod checker;
pub use self::{
    actions::{Action, AssignmentActions},
    checker::{is_array, value_check},
};
