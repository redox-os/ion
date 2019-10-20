use err_derive::Error;
#[cfg(target_os = "redox")]
use redox_users::All;
use std::{
    collections::VecDeque,
    env::{self, set_current_dir},
    io,
    path::{Component, Path, PathBuf},
};
#[cfg(not(target_os = "redox"))]
use users::os::unix::UserExt;

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
    fn normalize_path(&mut self, dir: &Path) -> PathBuf {
        // Create a clone of the current directory.
        let mut new_dir = match self.dirs.front() {
            Some(cur_dir) => cur_dir.clone(),
            None => PathBuf::new(),
        };

        // Iterate through components of the specified directory
        // and calculate the new path based on them.
        for component in dir.components() {
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
        self.dirs.rotate_right(num);
        self.set_current_dir_by_index(0)
    }

    // pushd +<num>
    pub fn rotate_left(&mut self, num: usize) -> Result<(), DirStackError> {
        self.dirs.rotate_left(num);
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

    pub fn change_and_push_dir(&mut self, dir: &Path) -> Result<(), DirStackError> {
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

        self.popd(0);
        println!("{}", prev);
        self.change_and_push_dir(Path::new(&prev))
    }

    pub fn switch_to_home_directory(&mut self) -> Result<(), DirStackError> {
        match env::var_os("HOME") {
            Some(home) => self.change_and_push_dir(Path::new(&home)),
            #[cfg(not(target_os = "redox"))]
            None => users::get_user_by_uid(users::get_current_uid())
                .map_or(Err(DirStackError::FailedFetchHome), |user| {
                    self.change_and_push_dir(user.home_dir())
                }),
            #[cfg(target_os = "redox")]
            None => {
                if let Ok(users) = redox_users::AllUsers::new(redox_users::Config::default()) {
                    redox_users::get_uid()
                        .ok()
                        .and_then(|id| users.get_by_id(id))
                        .map_or(Err(DirStackError::FailedFetchHome), |user| {
                            self.change_and_push_dir(Path::new(&user.home))
                        })
                } else {
                    Err(DirStackError::FailedFetchHome)
                }
            }
        }
    }

    pub fn swap(&mut self, index: usize) -> Result<(), DirStackError> {
        if self.dirs.len() <= index {
            return Err(DirStackError::NoOtherDir);
        }
        self.dirs.swap(0, index);
        self.set_current_dir_by_index(0)
    }

    pub fn pushd(&mut self, path: &Path, keep_front: bool) -> Result<(), DirStackError> {
        let index = if keep_front { 1 } else { 0 };
        let new_dir = self.normalize_path(path);
        self.insert_dir(index, new_dir);
        self.set_current_dir_by_index(index)
    }

    /// Attempts to set the current directory to the directory stack's previous directory,
    /// and then removes the front directory from the stack.
    pub fn popd(&mut self, index: usize) -> Option<PathBuf> { self.dirs.remove(index) }

    pub fn clear(&mut self) { self.dirs.truncate(1) }

    /// Create a new `DirectoryStack` containing the current working directory,
    /// if available.
    pub fn new() -> Self {
        let mut dirs: VecDeque<PathBuf> = VecDeque::new();
        if let Ok(curr_dir) = env::current_dir() {
            env::set_var("PWD", curr_dir.to_str().unwrap_or("?"));
            dirs.push_front(curr_dir);
        } else {
            eprintln!("ion: failed to get current directory when building directory stack");
            env::set_var("PWD", "?");
        }
        Self { dirs, max_depth: None }
    }
}
