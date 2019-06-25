use super::{super::types, Value};

/// The exit status of a command
///
/// Provides some helpers for defining builtins like error messages and semantic constants
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Status(i32);

impl Status {
    /// Failed to execute a given command (a parsing/expansion error occured)
    pub const COULD_NOT_EXEC: Self = Status(126);
    /// In builtins that output bools, indicates negation
    pub const FALSE: Self = Status(1);
    /// The command does not exist
    pub const NO_SUCH_COMMAND: Self = Status(127);
    /// The execution succeeded
    pub const SUCCESS: Self = Status(0);
    /// The process was killed
    pub const TERMINATED: Self = Status(143);
    /// In builtins that outputs bools, indicates that the result is true
    pub const TRUE: Self = Status(0);

    /// Make an exit code out of a signal
    pub fn from_signal(signal: u8) -> Self { Status(i32::from(128 + signal)) }

    /// From a raw exit code (native commands)
    pub fn from_exit_code(code: i32) -> Self { Status(code) }

    /// A generic error occured. Prints an helper text
    pub fn error<T: AsRef<str>>(err: T) -> Self {
        let err = err.as_ref();
        if !err.is_empty() {
            eprintln!("{}", err);
        }
        Status(1)
    }

    /// Wrong arguments submitted to the builtin
    pub fn bad_argument<T: AsRef<str>>(err: T) -> Self {
        let err = err.as_ref();
        if !err.is_empty() {
            eprintln!("{}", err);
        }
        Status(2)
    }

    /// Indicates if the operation is successful
    pub fn is_success(self) -> bool { self.0 == 0 }

    /// Indicates if the operation is unsuccessful
    pub fn is_failure(self) -> bool { self.0 != 0 }

    /// Convert to a raw OS exit code
    pub fn as_os_code(self) -> i32 { self.0 }

    /// Change true to false and false to true. Looses information
    pub fn toggle(&mut self) { self.0 = if self.is_success() { 1 } else { 0 }; }
}

impl<'a> From<Status> for Value<types::Function<'a>> {
    fn from(status: Status) -> Self { Value::Str(status.into()) }
}

impl From<Status> for types::Str {
    fn from(status: Status) -> Self { types::Str::from(status.as_os_code().to_string()) }
}

impl From<std::io::Result<()>> for Status {
    fn from(res: std::io::Result<()>) -> Self {
        match res {
            Ok(_) => Status::SUCCESS,
            Err(err) => Status::error(format!("{}", err)),
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
