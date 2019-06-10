#![allow(unknown_lints)]

use doc_comment::doctest;
use ion_braces as braces;
use ion_lexers as lexers;
use ion_ranges as ranges;
use ion_sys as sys;

#[macro_use]
pub mod types;
#[macro_use]
pub mod parser;
pub mod builtins;
mod memory;
mod shell;

doctest!("./description.md");

pub(crate) use self::memory::IonPool;
pub use crate::{
    builtins::{BuiltinFunction, BuiltinMap},
    shell::*,
};
