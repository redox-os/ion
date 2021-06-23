//! System specific shell variables for NULL_PATH

#[cfg(target_os = "redox")]
/// NULL_PATH on Redox OS
pub const NULL_PATH: &str = "null:";
#[cfg(all(unix, not(target_os = "redox")))]
/// NULL_PATH on Unix systems
pub const NULL_PATH: &str = "/dev/null";
