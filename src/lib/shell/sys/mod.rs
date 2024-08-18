//! System specific shell variables for NULL_PATH

#[cfg(unix)]
/// NULL_PATH on Unix systems
pub const NULL_PATH: &str = "/dev/null";
