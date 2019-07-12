#[cfg(target_os = "redox")]
pub const NULL_PATH: &str = "null:";
#[cfg(all(unix, not(target_os = "redox")))]
pub const NULL_PATH: &str = "/dev/null";
