use std::{
    fs::File, io, os::unix::io::{AsRawFd, FromRawFd, RawFd},
};
use sys;

/// Use dup2 to replace `old` with `new` using `old`s file descriptor ID
pub(crate) fn redir(old: RawFd, new: RawFd) {
    if let Err(e) = sys::dup2(old, new) {
        eprintln!("ion: could not duplicate {} to {}: {}", old, new, e);
    }
}

/// Duplicates STDIN, STDOUT, and STDERR; in that order; and returns them as `File`s.
/// Why, you ask? A simple safety mechanism to ensure that the duplicated FDs are closed
/// when dropped.
pub(crate) fn duplicate_streams() -> io::Result<(File, File, File)> {
    // Duplicates STDIN and converts it into a `File`.
    sys::dup(sys::STDIN_FILENO).map(|fd| unsafe { File::from_raw_fd(fd) })
        // Do the same for stdout, and then meld the result with stdin
        .and_then(|stdin| sys::dup(sys::STDOUT_FILENO)
            .map(|fd| unsafe { File::from_raw_fd(fd) })
            .map(|stdout| (stdin, stdout))
        )
        // And then meld stderr alongside stdin and stdout
        .and_then(|(stdin, stdout)| sys::dup(sys::STDERR_FILENO)
            .map(|fd| unsafe { File::from_raw_fd(fd) })
            .map(|stderr| (stdin, stdout, stderr))
        )
}

pub(crate) fn redirect_streams(inp: File, out: File, err: File) {
    redir(inp.as_raw_fd(), sys::STDIN_FILENO);
    redir(out.as_raw_fd(), sys::STDOUT_FILENO);
    redir(err.as_raw_fd(), sys::STDERR_FILENO);
}
