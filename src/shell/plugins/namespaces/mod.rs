#[cfg(target_os = "redox")]
mod redox;
#[cfg(target_os = "redox")]
pub use self::redox::*;

#[cfg(all(unix, not(target_os = "redox")))]
mod unix;
#[cfg(all(unix, not(target_os = "redox")))]
pub use self::unix::*;

use std::ffi::CString;
use std::fmt::{self, Display, Formatter};
use std::io;
use types::Identifier;

#[repr(C)]
#[derive(Debug)]
/// The foregein structure returned when executing a namespace plugin function.
///
/// This structure is a direct equivalent to `Option<CString>`. If the dynamic library from which
/// this result was returned was written in Rust, then the `data` pointer contained within was
/// created by calling `string.into_raw()` on a `CString`. In order to prevent a memory leak, this
/// structure should immediately be converted back into a `CString` by calling
/// `CString::from_raw()`.
struct NamespaceResult {
    exists: bool,
    data: *mut i8,
}

impl NamespaceResult {
    /// Converts the non-native structure into a proper, native Rust equivalent.
    /// The `exists` field indicates whether the `data` field was initialized or not.
    /// The `data` pointer is converted back into a native `CString` with `CString::from_raw()`.
    fn into_option(self) -> Option<CString> {
        if self.exists { Some(unsafe { CString::from_raw(self.data) }) } else { None }
    }
}

#[derive(Debug)]
/// A possible error that can be caused when attempting to obtain or execute a
/// function within a given namespace.
pub enum NamespaceError {
    /// This occurs when a symbol could not be loaded from the library in question. It is an
    /// error that infers that the problem is with the plugin, not Ion itself.
    SymbolErr(io::Error),
    /// Function names must be valid UTF-8. If they aren't something's wrong with the plugin.
    UTF8Function,
    /// The result from a plugin must be valid UTF-8. If it isn't, the plugin's bad.
    UTF8Result,
    /// This infers that the user called a function that doesn't exist in the library. Bad user, bad.
    FunctionMissing(Identifier),
}

impl Display for NamespaceError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            NamespaceError::SymbolErr(ref error) => write!(f, "symbol error: {}", error),
            NamespaceError::UTF8Function => write!(f, "function has invalid UTF-8 name"),
            NamespaceError::UTF8Result => write!(f, "result is not valid UTF-8"),
            NamespaceError::FunctionMissing(ref func) => write!(f, "{} doesn't exist in namespace", func),
        }
    }
}
