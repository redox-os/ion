//! # Ion - the pipe-oriented embedded language
//!
//! Ion is an embeddable shell for rust. This means your users can benefit from a fully-fledged
//! programming language to configure your application, rather than predefined layouts imposed by
//! formats like yaml. This also means the configuration can be completely responsive and react to
//! events in any way the user sees fit.
//!
//! ## Getting started
//!
//! ```toml
//! [dependencies]
//! ion_shell = "1.0"
//! ```
//!
//! ## Documentation
//!  - [Ion programming language manual](https://doc.redox-os.org/ion-manual/)
//!
//! ## Demo
//!
//! ```rust
//! use ion_shell::{builtins::Status, types, BuiltinFunction, BuiltinMap, Shell};
//! use std::{cell::RefCell, rc::Rc, thread, time};
//!
//! enum Layout {
//!     Simple,
//!     Complex(String),
//! }
//!
//! fn main() {
//!     let mut i = 0;
//!     let layout = RefCell::new(Layout::Simple); // A state for your application
//!
//!     // Create a custom callback to update your state when called by a script
//!     let set_layout: BuiltinFunction = &move |args: &[types::Str], shell: &mut Shell| {
//!         *layout.borrow_mut() = if let Some(text) = args.get(0) {
//!             Layout::Complex(text.to_string())
//!         } else {
//!             Layout::Simple
//!         };
//!         Status::SUCCESS
//!     };
//!
//!     // Create a shell
//!     let mut shell = Shell::new();
//!
//!     // Register the builtins
//!     shell.builtins_mut().add("layout", set_layout, "Set the application layout");
//!
//!     // Read a file and execute it
//!     shell.execute_file("/home/user/.config/my-application/config.ion");
//!
//!     for _ in 0..255 {
//!         i += 1;
//!         // call a user-defined callback function named on_update
//!         let _ = shell.execute_function("on_update", &["ion", &i.to_string()]);
//!
//!         thread::sleep(time::Duration::from_millis(5));
//!     }
//! }
//! ```

#![allow(unknown_lints)]
#![warn(missing_docs)]
use ion_ranges as ranges;

/// The various types used for storing values
#[macro_use]
pub mod types;
/// Direct access to the parsers
#[macro_use]
pub mod parser;
mod assignments;
/// Access to the predefined builtins
pub mod builtins;
/// Expand the AST to create pipelines
pub mod expansion;
mod memory;
mod shell;

pub(crate) use self::memory::IonPool;
pub use crate::{
    builtins::{BuiltinFunction, BuiltinMap},
    shell::*,
};
pub use builtins_proc::builtin;
