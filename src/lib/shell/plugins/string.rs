use std::{
    fmt::{self, Display, Formatter},
    io,
};
use types;

#[derive(Debug)]
/// A possible error that can be caused when attempting to obtain or execute a
/// function that is supposed to return a string from across the FFI boundaries.
pub(crate) enum StringError {
    /// This occurs when a symbol could not be loaded from the library in question. It is an
    /// error that infers that the problem is with the plugin, not Ion itself.
    SymbolErr(io::Error),
    /// Function names must be valid UTF-8. If they aren't something's wrong
    /// with the plugin.
    UTF8Function,
    /// The result from a plugin must be valid UTF-8. If it isn't, the plugin's
    /// bad.
    UTF8Result,
    /// This infers that the user called a function that doesn't exist in the library. Bad
    /// user, bad.
    FunctionMissing(types::Str),
}

impl Display for StringError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            StringError::SymbolErr(ref error) => write!(f, "symbol error: {}", error),
            StringError::UTF8Function => write!(f, "function has invalid UTF-8 name"),
            StringError::UTF8Result => write!(f, "result is not valid UTF-8"),
            StringError::FunctionMissing(ref func) => {
                write!(f, "{} doesn't exist in namespace", func)
            }
        }
    }
}
