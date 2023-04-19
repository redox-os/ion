use super::{super::types, Value};
use std::{fmt::Display, rc::Rc};

/// The exit status of a command
///
/// Provides some helpers for defining builtins like error messages and semantic constants
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Status(i32);

impl Status {
    /// Failed to execute a given command (a parsing/expansion error occured)
    pub const COULD_NOT_EXEC: Self = Self(126);
    /// In builtins that output bools, indicates negation
    pub const FALSE: Self = Self(1);
    /// The command does not exist
    pub const NO_SUCH_COMMAND: Self = Self(127);
    /// The execution succeeded
    pub const SUCCESS: Self = Self(0);
    /// The process was killed
    pub const TERMINATED: Self = Self(143);
    /// In builtins that outputs bools, indicates that the result is true
    pub const TRUE: Self = Self(0);

    /// Make an exit code out of a signal
    pub fn from_signal(signal: u8) -> Self { Self(i32::from(128 + signal)) }

    /// From a raw exit code (native commands)
    pub const fn from_exit_code(code: i32) -> Self { Self(code) }

    /// A generic error occured. Prints an helper text
    pub fn error<T: AsRef<str>>(err: T) -> Self {
        let err = err.as_ref();
        if !err.is_empty() {
            eprintln!("{}", err);
        }
        Self(1)
    }

    /// Wrong arguments submitted to the builtin
    pub fn bad_argument<T: AsRef<str>>(err: T) -> Self {
        let err = err.as_ref();
        if !err.is_empty() {
            eprintln!("{}", err);
        }
        Self(2)
    }

    /// Indicates if the operation is successful
    pub const fn is_success(self) -> bool { self.0 == 0 }

    /// Indicates if the operation is unsuccessful
    pub const fn is_failure(self) -> bool { self.0 != 0 }

    /// Convert to a raw OS exit code
    pub const fn as_os_code(self) -> i32 { self.0 }

    /// Change true to false and false to true. Looses information
    pub fn toggle(&mut self) { self.0 = if self.is_success() { 1 } else { 0 }; }
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
}

impl From<Status> for Value<Rc<types::Function>> {
    fn from(status: Status) -> Self { Self::Str(status.into()) }
}

impl From<Status> for types::Str {
    fn from(status: Status) -> Self { status.as_os_code().to_string().into() }
}

impl From<std::io::Result<()>> for Status {
    fn from(res: std::io::Result<()>) -> Self {
        if let Err(err) = res {
            Self::error(format!("{}", err))
        } else {
            Self::SUCCESS
        }
    }
}

impl From<bool> for Status {
    fn from(success: bool) -> Self {
        if success {
            Self::TRUE
        } else {
            Self::FALSE
        }
    }
}
