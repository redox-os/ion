use crate::PipelineError;
use nix::unistd;
use std::{
    fs::File,
    io,
    os::unix::io::{AsRawFd, FromRawFd},
};

/// Use dup2 to replace `old` with `new` using `old`s file descriptor ID
fn redir<F: AsRawFd>(old: &Option<File>, new: &F) -> Result<(), PipelineError> {
    if let Some(old) = old.as_ref().map(AsRawFd::as_raw_fd) {
        unistd::dup2(old, new.as_raw_fd()).map_err(PipelineError::CloneFdFailed)?;
    }
    Ok(())
}

/// Duplicates STDIN, STDOUT, and STDERR; in that order; and returns them as `File`s.
/// Why, you ask? A simple safety mechanism to ensure that the duplicated FDs are closed
/// when dropped.
pub fn duplicate() -> nix::Result<(Option<File>, File, File)> {
    // STDIN may have been closed for a background shell, so it is ok if it cannot be duplicated.
    let stdin =
        unistd::dup(io::stdin().as_raw_fd()).ok().map(|fd| unsafe { File::from_raw_fd(fd) });

    let stdout = unsafe { File::from_raw_fd(unistd::dup(io::stdout().as_raw_fd())?) };
    let stderr = unsafe { File::from_raw_fd(unistd::dup(io::stderr().as_raw_fd())?) };
    // And then meld stderr alongside stdin and stdout
    Ok((stdin, stdout, stderr))
}

#[inline]
pub fn redirect(
    inp: &Option<File>,
    out: &Option<File>,
    err: &Option<File>,
) -> Result<(), PipelineError> {
    redir(inp, &io::stdin())?;
    redir(out, &io::stdout())?;
    redir(err, &io::stderr())
}
