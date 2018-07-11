use std::fs::ReadDir;
use types;

pub(crate) struct Library;

/// Grabs all `Library` entries found within a given directory
pub(crate) struct LibraryIterator {
    directory: ReadDir,
}

impl LibraryIterator {
    pub(crate) fn new(directory: ReadDir) -> LibraryIterator { LibraryIterator { directory } }
}

impl Iterator for LibraryIterator {
    // The `Identifier` is the name of the namespace for which values may be pulled.
    // The `Library` is a handle to dynamic library loaded into memory.
    type Item = (types::Str, Library);

    fn next(&mut self) -> Option<(types::Str, Library)> { None }
}
