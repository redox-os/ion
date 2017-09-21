pub(crate) const SUCCESS: i32 = 0;
pub(crate) const FAILURE: i32 = 1;
pub(crate) const BAD_ARG: i32 = 2;
pub(crate) const COULD_NOT_EXEC: i32 = 126;
pub(crate) const NO_SUCH_COMMAND: i32 = 127;
pub(crate) const TERMINATED: i32 = 143;

pub(crate) fn get_signal_code(signal: i32) -> i32 { 128 + signal }
