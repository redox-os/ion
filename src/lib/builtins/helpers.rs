use super::{super::types, Value};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Status(i32);

impl Status {
    pub const COULD_NOT_EXEC: Self = Status(126);
    pub const FALSE: Self = Status(1);
    pub const NO_SUCH_COMMAND: Self = Status(127);
    pub const SUCCESS: Self = Status(0);
    pub const TERMINATED: Self = Status(143);
    pub const TRUE: Self = Status(0);

    pub fn from_signal(signal: i32) -> Self { Status(128 + signal) }

    pub fn from_exit_code(code: i32) -> Self { Status(code) }

    pub fn from_bool(b: bool) -> Self { Status(!b as i32) }

    pub fn error<T: AsRef<str>>(err: T) -> Self {
        let err = err.as_ref();
        if !err.is_empty() {
            eprintln!("{}", err);
        }
        Status(1)
    }

    pub fn bad_argument<T: AsRef<str>>(err: T) -> Self {
        let err = err.as_ref();
        if !err.is_empty() {
            eprintln!("{}", err);
        }
        Status(2)
    }

    pub fn is_success(self) -> bool { self.0 == 0 }

    pub fn is_failure(self) -> bool { self.0 != 0 }

    pub fn as_os_code(self) -> i32 { self.0 }

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
