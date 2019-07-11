pub mod signals;

#[cfg(target_os = "redox")]
pub const NULL_PATH: &str = "null:";
#[cfg(unix)]
pub const NULL_PATH: &str = "/dev/null";
