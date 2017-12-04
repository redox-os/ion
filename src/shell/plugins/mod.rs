pub mod methods;
pub mod namespaces;
mod library_iter;
mod string;

pub(crate) use self::library_iter::*;
pub(crate) use self::string::StringError;

use std::path::PathBuf;
use xdg::BaseDirectories;

pub(crate) fn config_dir() -> Option<PathBuf> {
    match BaseDirectories::with_prefix("ion") {
        Ok(base_dirs) => {
            match base_dirs.create_config_directory("plugins") {
                Ok(mut path) => {
                    Some(path)
                },
                Err(err) => {
                    eprintln!("ion: unable to create config directory: {:?}", err);
                    None
                }
            }
        },
        Err(err) => {
            eprintln!("ion: unable to get config directory: {:?}", err);
            None
        }
    }
}
