#[cfg(target_os = "redox")]
#[path = "redox/mod.rs"]
mod sys;

#[cfg(unix)]
#[path = "unix/mod.rs"]
mod sys;

mod shared;

pub use self::{shared::*, sys::*};
