use super::{
    super::job::{RefinedJob, TeeItem},
    PipelineError,
};

#[cfg(any(target_os = "ios", target_os = "macos"))]
use nix::fcntl::{fcntl, FcntlArg};
use nix::{fcntl::OFlag, unistd};
use std::{fs::File, os::unix::io::FromRawFd};

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
pub fn create_pipe() -> Result<(File, File), PipelineError> {
    let (reader, writer) =
        unistd::pipe2(OFlag::O_CLOEXEC).map_err(PipelineError::CreatePipeError)?;
    Ok(unsafe { (File::from_raw_fd(reader), File::from_raw_fd(writer)) })
}
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub fn create_pipe() -> Result<(File, File), PipelineError> {
    let (reader, writer) = unistd::pipe().map_err(PipelineError::CreatePipeError)?;
    fcntl(reader, FcntlArg::F_SETFL(OFlag::O_CLOEXEC)).map_err(PipelineError::CreatePipeError)?;
    fcntl(writer, FcntlArg::F_SETFL(OFlag::O_CLOEXEC)).map_err(PipelineError::CreatePipeError)?;
    Ok(unsafe { (File::from_raw_fd(reader), File::from_raw_fd(writer)) })
}

pub struct TeePipe<'a, 'b> {
    parent:          &'a mut RefinedJob<'b>,
    ext_stdio_pipes: &'a mut Option<Vec<File>>,
    is_external:     bool,
}

impl<'a, 'b> TeePipe<'a, 'b> {
    pub fn new(
        parent: &'a mut RefinedJob<'b>,
        ext_stdio_pipes: &'a mut Option<Vec<File>>,
        is_external: bool,
    ) -> Self {
        TeePipe { parent, ext_stdio_pipes, is_external }
    }

    fn inner_connect<F>(&mut self, tee: &mut TeeItem, mut action: F) -> Result<(), PipelineError>
    where
        F: FnMut(&mut RefinedJob<'b>, File),
    {
        let (reader, writer) = create_pipe()?;
        (*tee).source = Some(reader);
        if self.is_external {
            self.ext_stdio_pipes
                .get_or_insert_with(|| Vec::with_capacity(4))
                .push(writer.try_clone().map_err(PipelineError::ClonePipeFailed)?);
        }
        action(self.parent, writer);
        Ok(())
    }

    pub fn connect(&mut self, out: &mut TeeItem, err: &mut TeeItem) -> Result<(), PipelineError> {
        self.inner_connect(out, RefinedJob::stdout)?;
        self.inner_connect(err, RefinedJob::stderr)
    }
}
