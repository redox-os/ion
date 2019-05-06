use crate::sys;
use std::{
    fs::File,
    io,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
};

/// Use dup2 to replace `old` with `new` using `old`s file descriptor ID
fn redir(old: &Option<File>, new: RawFd) {
    if let Some(old) = old.as_ref().map(AsRawFd::as_raw_fd) {
        if let Err(e) = sys::dup2(old, new) {
            eprintln!("ion: could not duplicate {} to {}: {}", old, new, e);
        }
    }
}

/// Duplicates STDIN, STDOUT, and STDERR; in that order; and returns them as `File`s.
/// Why, you ask? A simple safety mechanism to ensure that the duplicated FDs are closed
/// when dropped.
pub(crate) fn duplicate_streams() -> io::Result<(Option<File>, File, File)> {
    // STDIN may have been closed for a background shell, so it is ok if it cannot be duplicated.
    let stdin = sys::dup(sys::STDIN_FILENO).ok().map(|fd| unsafe { File::from_raw_fd(fd) });

    let stdout = unsafe { File::from_raw_fd(sys::dup(sys::STDOUT_FILENO)?) };
    let stderr = unsafe { File::from_raw_fd(sys::dup(sys::STDERR_FILENO)?) };
    // And then meld stderr alongside stdin and stdout
    Ok((stdin, stdout, stderr))
}

#[inline]
pub(crate) fn redirect_streams(inp: &Option<File>, out: &Option<File>, err: &Option<File>) {
    redir(inp, sys::STDIN_FILENO);
    redir(out, sys::STDOUT_FILENO);
    redir(err, sys::STDERR_FILENO);
}
