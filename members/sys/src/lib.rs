#[cfg(all(unix, not(target_os = "redox")))]
extern crate libc;
#[cfg(all(unix, not(target_os = "redox")))]
extern crate users as users_unix;

#[cfg(target_os = "redox")]
extern crate syscall;

#[cfg(target_os = "redox")]
#[path = "sys/redox/mod.rs"]
mod sys;

#[cfg(unix)]
#[path = "sys/unix/mod.rs"]
mod sys;

pub use self::sys::*;
