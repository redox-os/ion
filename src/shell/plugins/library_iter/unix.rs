use libloading::Library;
use std::fs::ReadDir;
use types::Identifier;

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
    type Item = (Identifier, Library);

    fn next(&mut self) -> Option<(Identifier, Library)> {
        while let Some(entry) = self.directory.next() {
            let entry = if let Ok(entry) = entry { entry } else { continue };
            let path = entry.path();
            // An entry is a library if it is a file with a 'so' extension.
            if path.is_file() && path.extension().map_or(false, |ext| ext == "so") {
                // The identifier will be the file name of that file, without the extension.
                let identifier = match path.file_stem().unwrap().to_str() {
                    Some(filename) => Identifier::from(filename),
                    None => {
                        eprintln!("ion: namespace plugin has invalid filename");
                        continue;
                    }
                };

                // This will attempt to load the library into memory.
                match Library::new(path.as_os_str()) {
                    Ok(library) => return Some((identifier, library)),
                    Err(why) => {
                        eprintln!("ion: failed to load library: {:?}, {:?}", path, why);
                        continue;
                    }
                }
            } else {
                continue;
            }
        }
        None
    }
}
