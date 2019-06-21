use super::{
    super::{
        job::{RefinedJob, TeeItem},
        sys,
    },
    PipelineError,
};

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

    fn inner_connect<F>(&mut self, tee: &mut TeeItem, mut action: F) -> Result<(), PipelineError>
    where
        F: FnMut(&mut RefinedJob<'b>, File),
    {
        let (reader, writer) =
            sys::pipe2(libc::O_CLOEXEC).map_err(PipelineError::CreatePipeError)?;
        (*tee).source = Some(unsafe { File::from_raw_fd(reader) });
        action(self.parent, unsafe { File::from_raw_fd(writer) });
        if self.is_external {
            self.ext_stdio_pipes
                .get_or_insert_with(|| Vec::with_capacity(4))
                .push(unsafe { File::from_raw_fd(writer) });
        }
        Ok(())
    }

    pub fn connect(&mut self, out: &mut TeeItem, err: &mut TeeItem) -> Result<(), PipelineError> {
        self.inner_connect(out, RefinedJob::stdout)?;
        self.inner_connect(err, RefinedJob::stderr)
    }
}
