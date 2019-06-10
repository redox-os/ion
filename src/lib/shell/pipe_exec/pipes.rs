use super::{
    super::job::{RefinedJob, TeeItem},
    append_external_stdio_pipe, PipeError,
};

use crate::sys;
use std::{fs::File, os::unix::io::FromRawFd};

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

    fn inner_connect<F>(&mut self, tee: &mut TeeItem, mut action: F) -> Result<(), PipeError>
    where
        F: FnMut(&mut RefinedJob<'b>, File),
    {
        let (reader, writer) =
            sys::pipe2(sys::O_CLOEXEC).map_err(|cause| PipeError::CreatePipeError { cause })?;
        (*tee).source = Some(unsafe { File::from_raw_fd(reader) });
        action(self.parent, unsafe { File::from_raw_fd(writer) });
        if self.is_external {
            append_external_stdio_pipe(self.ext_stdio_pipes, writer);
        }
        Ok(())
    }

    pub fn connect(&mut self, out: &mut TeeItem, err: &mut TeeItem) -> Result<(), PipeError> {
        self.inner_connect(out, RefinedJob::stdout)?;
        self.inner_connect(err, RefinedJob::stderr)
    }
}
