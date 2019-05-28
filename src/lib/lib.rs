#![allow(unknown_lints)]

#[macro_use]
extern crate err_derive;
extern crate ion_braces as braces;
extern crate ion_lexers as lexers;
extern crate ion_ranges as ranges;
extern crate ion_sys as sys;

#[macro_use]
pub mod types;
#[macro_use]
pub mod parser;
pub mod builtins;
mod memory;
mod shell;

pub(crate) use self::memory::IonPool;
pub use crate::shell::*;
