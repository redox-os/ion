#[macro_use]
extern crate bitflags;

pub mod assignments;
pub mod arguments;
pub mod designators;

pub use self::{assignments::*, arguments::*, designators::*};
