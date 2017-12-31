#[cfg(target_os = "redox")]
mod redox;
// #[cfg(target_os = "redox")]
// pub(crate) use self::redox::*;

#[cfg(all(unix, not(target_os = "redox")))]
mod unix;
#[cfg(all(unix, not(target_os = "redox")))]
pub(crate) use self::unix::*;
