use libloading::Library;
use std::fs::ReadDir;
use types::Identifier;


/// Grabs `Library` entries found within a given directory
pub struct LibraryIterator {
    directory: ReadDir,
}

impl LibraryIterator {
    pub fn new(directory: ReadDir) -> LibraryIterator { LibraryIterator { directory } }
}

impl Iterator for LibraryIterator {
    type Item = (Identifier, Library);

    fn next(&mut self) -> Option<(Identifier, Library)> {
        while let Some(entry) = self.directory.next() {
            let entry = if let Ok(entry) = entry { entry } else { continue };
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "so") {
                let identifier = match path.file_stem().unwrap().to_str() {
                    Some(filename) => Identifier::from(filename),
                    None => {
                        eprintln!("ion: namespace plugin has invalid filename");
                        continue;
                    }
                };

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
