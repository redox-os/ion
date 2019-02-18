use super::{
    super::job::{RefinedJob, TeeItem},
    append_external_stdio_pipe, pipe_fail,
};

use crate::sys;
use std::{fs::File, os::unix::io::FromRawFd};

pub(crate) struct TeePipe<'a> {
    parent:          &'a mut RefinedJob,
    ext_stdio_pipes: &'a mut Option<Vec<File>>,
    is_external:     bool,
}

impl<'a> TeePipe<'a> {
    pub(crate) fn new(
        parent: &'a mut RefinedJob,
        ext_stdio_pipes: &'a mut Option<Vec<File>>,
        is_external: bool,
    ) -> TeePipe<'a> {
        TeePipe { parent, ext_stdio_pipes, is_external }
    }

    fn inner_connect<F>(&mut self, tee: &mut TeeItem, mut action: F)
    where
        F: FnMut(&mut RefinedJob, File),
    {
        match sys::pipe2(sys::O_CLOEXEC) {
            Err(e) => pipe_fail(e),
            Ok((reader, writer)) => {
                (*tee).source = Some(unsafe { File::from_raw_fd(reader) });
                action(self.parent, unsafe { File::from_raw_fd(writer) });
                if self.is_external {
                    append_external_stdio_pipe(self.ext_stdio_pipes, writer);
                }
            }
        }
    }

    pub(crate) fn connect(&mut self, out: &mut TeeItem, err: &mut TeeItem) {
        self.inner_connect(out, |parent, writer| parent.stdout(writer));
        self.inner_connect(err, |parent, writer| parent.stderr(writer));
    }
}
