pub const SUCCESS: i32 = 0;
pub const FAILURE: i32 = 1;
pub const BAD_ARG: i32 = 2;
pub const COULD_NOT_EXEC: i32 = 126;
pub const NO_SUCH_COMMAND: i32 = 127;
pub const TERMINATED: i32 = 143;

pub fn get_signal_code(signal: i32) -> i32 { 128 + signal }
