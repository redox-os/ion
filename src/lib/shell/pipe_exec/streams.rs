use nix::unistd;
use std::{
    fs::File,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
};

/// Use dup2 to replace `old` with `new` using `old`s file descriptor ID
fn redir(old: &Option<File>, new: RawFd) {
    if let Some(old) = old.as_ref().map(AsRawFd::as_raw_fd) {
        if let Err(e) = unistd::dup2(old, new) {
            eprintln!("ion: could not duplicate {} to {}: {}", old, new, e);
        }
    }
}

/// Duplicates STDIN, STDOUT, and STDERR; in that order; and returns them as `File`s.
/// Why, you ask? A simple safety mechanism to ensure that the duplicated FDs are closed
/// when dropped.
pub fn duplicate() -> nix::Result<(Option<File>, File, File)> {
    // STDIN may have been closed for a background shell, so it is ok if it cannot be duplicated.
    let stdin =
        unistd::dup(nix::libc::STDIN_FILENO).ok().map(|fd| unsafe { File::from_raw_fd(fd) });

    let stdout = unsafe { File::from_raw_fd(unistd::dup(nix::libc::STDOUT_FILENO)?) };
    let stderr = unsafe { File::from_raw_fd(unistd::dup(nix::libc::STDERR_FILENO)?) };
    // And then meld stderr alongside stdin and stdout
    Ok((stdin, stdout, stderr))
}

#[inline]
pub fn redirect(inp: &Option<File>, out: &Option<File>, err: &Option<File>) {
    redir(inp, nix::libc::STDIN_FILENO);
    redir(out, nix::libc::STDOUT_FILENO);
    redir(err, nix::libc::STDERR_FILENO);
}
