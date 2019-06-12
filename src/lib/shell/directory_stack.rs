use crate::sys::env as sys_env;
use err_derive::Error;
use std::{
    collections::VecDeque,
    env::{self, set_current_dir},
    io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Error)]
pub enum DirStackError {
    #[error(display = "index '{}' out of range", index)]
    OutOfRange { index: usize },
    #[error(display = "failed to get home directory")]
    FailedFetchHome,
    #[error(display = "failed to convert home directory to str")]
    PathConversionFailed,
    #[error(display = "failed to set current dir to {}: {}", dir, cause)]
    DirChangeFailure { dir: String, cause: io::Error },
    #[error(display = "no previous directory to switch to")]
    NoPreviousDir,
    #[error(display = "no directory to switch with")]
    NoOtherDir,
}

fn set_current_dir_ion(dir: &Path) -> Result<(), DirStackError> {
    set_current_dir(dir).map_err(|cause| DirStackError::DirChangeFailure {
        cause,
        dir: dir.to_string_lossy().into(),
    })?;

    env::set_var(
        "OLDPWD",
        env::var("PWD")
            .ok()
            .and_then(|pwd| if pwd.is_empty() { None } else { Some(pwd) })
            .unwrap_or_else(|| "?".into()),
    );

    env::set_var("PWD", dir.to_str().unwrap_or("?"));
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryStack {
    dirs:      VecDeque<PathBuf>, // The top is always the current directory
    max_depth: Option<usize>,
}

impl Default for DirectoryStack {
    fn default() -> Self { Self::new() }
}

impl DirectoryStack {
    fn normalize_path(&mut self, dir: &str) -> PathBuf {
        // Create a clone of the current directory.
        let mut new_dir = match self.dirs.front() {
            Some(cur_dir) => cur_dir.clone(),
            None => PathBuf::new(),
        };

        // Iterate through components of the specified directory
        // and calculate the new path based on them.
        for component in Path::new(dir).components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    new_dir.pop();
                }
                _ => {
                    new_dir.push(component);
                }
            };
        }

        new_dir
    }

    pub fn set_max_depth(&mut self, max_depth: Option<usize>) { self.max_depth = max_depth; }

    pub fn max_depth(&mut self) -> Option<usize> { self.max_depth }

    // pushd -<num>
    pub fn rotate_right(&mut self, num: usize) -> Result<(), DirStackError> {
        let len = self.dirs.len();
        self.rotate_left(len - (num % len))
    }

    // pushd +<num>
    pub fn rotate_left(&mut self, num: usize) -> Result<(), DirStackError> {
        for _ in 0..num {
            if let Some(popped_front) = self.dirs.pop_front() {
                self.dirs.push_back(popped_front);
            }
        }
        self.set_current_dir_by_index(0)
    }

    // sets current_dir to the element referred by index
    pub fn set_current_dir_by_index(&self, index: usize) -> Result<(), DirStackError> {
        let dir = self.dirs.get(index).ok_or_else(|| DirStackError::OutOfRange { index })?;

        set_current_dir_ion(dir)
    }

    pub fn dir_from_bottom(&self, num: usize) -> Option<&PathBuf> {
        self.dirs.get(self.dirs.len() - num)
    }

    pub fn dir_from_top(&self, num: usize) -> Option<&PathBuf> { self.dirs.get(num) }

    pub fn dirs(&self) -> impl DoubleEndedIterator<Item = &PathBuf> + ExactSizeIterator {
        self.dirs.iter()
    }

    fn insert_dir(&mut self, index: usize, path: PathBuf) {
        self.dirs.insert(index, path);
        if let Some(max_depth) = self.max_depth {
            self.dirs.truncate(max_depth);
        }
    }

    fn push_dir(&mut self, path: PathBuf) {
        self.dirs.push_front(path);
        if let Some(max_depth) = self.max_depth {
            self.dirs.truncate(max_depth);
        }
    }

    pub fn change_and_push_dir(&mut self, dir: &str) -> Result<(), DirStackError> {
        let new_dir = self.normalize_path(dir);
        set_current_dir_ion(&new_dir)?;
        self.push_dir(new_dir);
        Ok(())
    }

    fn get_previous_dir(&self) -> Option<String> {
        env::var("OLDPWD").ok().filter(|pwd| !pwd.is_empty() && pwd != "?")
    }

    pub fn switch_to_previous_directory(&mut self) -> Result<(), DirStackError> {
        let prev = self.get_previous_dir().ok_or(DirStackError::NoPreviousDir)?;

        self.dirs.remove(0);
        println!("{}", prev);
        self.change_and_push_dir(&prev)
    }

    pub fn switch_to_home_directory(&mut self) -> Result<(), DirStackError> {
        sys_env::home_dir().map_or(Err(DirStackError::FailedFetchHome), |home| {
            home.to_str().map_or(Err(DirStackError::PathConversionFailed), |home| {
                self.change_and_push_dir(home)
            })
        })
    }

    pub fn swap(&mut self, index: usize) -> Result<(), DirStackError> {
        if self.dirs.len() <= index {
            return Err(DirStackError::NoOtherDir);
        }
        self.dirs.swap(0, index);
        self.set_current_dir_by_index(0)
    }

    pub fn pushd(&mut self, path: PathBuf, keep_front: bool) -> Result<(), DirStackError> {
        let index = if keep_front { 1 } else { 0 };
        let new_dir = self.normalize_path(path.to_str().unwrap());
        self.insert_dir(index, new_dir);
        self.set_current_dir_by_index(index)
    }

    /// Attempts to set the current directory to the directory stack's previous directory,
    /// and then removes the front directory from the stack.
    pub fn popd(&mut self, index: usize) -> Option<PathBuf> { self.dirs.remove(index) }

    pub fn clear(&mut self) { self.dirs.truncate(1) }

    /// Create a new `DirectoryStack` containing the current working directory,
    /// if available.
    pub fn new() -> DirectoryStack {
        let mut dirs: VecDeque<PathBuf> = VecDeque::new();
        match env::current_dir() {
            Ok(curr_dir) => {
                env::set_var("PWD", curr_dir.to_str().unwrap_or("?"));
                dirs.push_front(curr_dir);
            }
            Err(_) => {
                eprintln!("ion: failed to get current directory when building directory stack");
                env::set_var("PWD", "?");
            }
        }
        DirectoryStack { dirs, max_depth: None }
    }
}
