#[cfg(target_os = "redox")]
#[path = "redox/mod.rs"]
mod sys;

#[cfg(unix)]
#[path = "unix/mod.rs"]
mod sys;

pub use self::sys::*;
