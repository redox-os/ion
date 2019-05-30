use super::variables::{Value, Variables};
use crate::sys::env as sys_env;
use std::{
    borrow::Cow,
    collections::VecDeque,
    env::{self, set_current_dir},
    path::{Component, Path, PathBuf},
};

fn set_current_dir_ion(dir: &Path) -> Result<(), Cow<'static, str>> {
    set_current_dir(dir).map_err(|why| Cow::Owned(format!("{}", why)))?;

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

#[derive(Debug)]
pub struct DirectoryStack {
    dirs: VecDeque<PathBuf>, // The top is always the current directory
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

    // pushd -<num>
    pub fn rotate_right(&mut self, num: usize) -> Result<(), Cow<'static, str>> {
        let len = self.dirs.len();
        self.rotate_left(len - (num % len))
    }

    // pushd +<num>
    pub fn rotate_left(&mut self, num: usize) -> Result<(), Cow<'static, str>> {
        for _ in 0..num {
            if let Some(popped_front) = self.dirs.pop_front() {
                self.dirs.push_back(popped_front);
            }
        }
        self.set_current_dir_by_index(0)
    }

    // sets current_dir to the element referred by index
    pub fn set_current_dir_by_index(&self, index: usize) -> Result<(), Cow<'static, str>> {
        let dir = self
            .dirs
            .get(index)
            .ok_or_else(|| Cow::Owned(format!("{}: directory stack out of range", index)))?;

        set_current_dir_ion(dir)
    }

    pub fn dir_from_bottom(&self, num: usize) -> Option<&PathBuf> {
        self.dirs.get(self.dirs.len() - num)
    }

    pub fn dir_from_top(&self, num: usize) -> Option<&PathBuf> { self.dirs.get(num) }

    pub fn dirs(&self) -> impl DoubleEndedIterator<Item = &PathBuf> + ExactSizeIterator {
        self.dirs.iter()
    }

    fn insert_dir(&mut self, index: usize, path: PathBuf, variables: &Variables) {
        self.dirs.insert(index, path);
        self.dirs.truncate(DirectoryStack::get_size(variables));
    }

    fn push_dir(&mut self, path: PathBuf, variables: &Variables) {
        self.dirs.push_front(path);
        self.dirs.truncate(DirectoryStack::get_size(variables));
    }

    fn change_and_push_dir(
        &mut self,
        dir: &str,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>> {
        let new_dir = self.normalize_path(dir);
        set_current_dir_ion(&new_dir).map_err(|err| {
            Cow::Owned(format!(
                "ion: failed to set current dir to {}: {}",
                new_dir.to_string_lossy(),
                err
            ))
        })?;
        self.push_dir(new_dir, variables);
        Ok(())
    }

    fn get_previous_dir(&self) -> Option<String> {
        env::var("OLDPWD").ok().filter(|pwd| !pwd.is_empty() && pwd != "?")
    }

    fn switch_to_previous_directory(
        &mut self,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>> {
        self.get_previous_dir()
            .ok_or(Cow::Borrowed("ion: no previous directory to switch to"))
            .and_then(|prev| {
                self.dirs.remove(0);
                println!("{}", prev);
                self.change_and_push_dir(&prev, variables)
            })
    }

    fn switch_to_home_directory(&mut self, variables: &Variables) -> Result<(), Cow<'static, str>> {
        sys_env::home_dir().map_or(
            Err(Cow::Borrowed("ion: failed to get home directory")),
            |home| {
                home.to_str().map_or(
                    Err(Cow::Borrowed("ion: failed to convert home directory to str")),
                    |home| self.change_and_push_dir(home, variables),
                )
            },
        )
    }

    pub fn cd<T: AsRef<str>>(
        &mut self,
        dir: Option<T>,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>> {
        match dir {
            Some(dir) => {
                let dir = dir.as_ref();
                if let Some(Value::Array(cdpath)) = variables.get_ref("CDPATH") {
                    if dir == "-" {
                        self.switch_to_previous_directory(variables)
                    } else {
                        let check_cdpath_first = cdpath
                            .iter()
                            .map(|path| {
                                let path_dir = format!("{}/{}", path, dir);
                                self.change_and_push_dir(&path_dir, variables)
                            })
                            .find(Result::is_ok)
                            .unwrap_or_else(|| self.change_and_push_dir(dir, variables));
                        self.dirs.remove(1);
                        check_cdpath_first
                    }
                } else {
                    self.change_and_push_dir(dir, variables)
                }
            }
            None => self.switch_to_home_directory(variables),
        }
    }

    pub fn swap(&mut self, index: usize) -> Result<(), Cow<'static, str>> {
        if self.dirs.len() <= index {
            return Err(Cow::Borrowed("no other directory"));
        }
        self.dirs.swap(0, index);
        self.set_current_dir_by_index(0)
    }

    pub fn pushd(
        &mut self,
        path: PathBuf,
        keep_front: bool,
        variables: &mut Variables,
    ) -> Result<(), Cow<'static, str>> {
        let index = if keep_front { 1 } else { 0 };
        let new_dir = self.normalize_path(path.to_str().unwrap());
        self.insert_dir(index, new_dir, variables);
        self.set_current_dir_by_index(index)
    }

    /// Attempts to set the current directory to the directory stack's previous directory,
    /// and then removes the front directory from the stack.
    pub fn popd(&mut self, index: usize) -> Option<PathBuf> { self.dirs.remove(index) }

    pub fn clear(&mut self) { self.dirs.truncate(1) }

    /// This function will take a map of variables as input and attempt to parse the value of
    /// the
    /// directory stack size variable. If it succeeds, it will return the value of that
    /// variable,
    /// else it will return a default value of 1000.
    fn get_size(variables: &Variables) -> usize {
        variables.get_str_or_empty("DIRECTORY_STACK_SIZE").parse::<usize>().unwrap_or(1000)
    }

    /// Create a new `DirectoryStack` containing the current working directory,
    /// if available.
    pub fn new() -> DirectoryStack {
        let mut dirs: VecDeque<PathBuf> = VecDeque::new();
        match env::current_dir() {
            Ok(curr_dir) => {
                env::set_var("PWD", curr_dir.to_str().unwrap_or_else(|| "?"));
                dirs.push_front(curr_dir);
            }
            Err(_) => {
                eprintln!("ion: failed to get current directory when building directory stack");
                env::set_var("PWD", "?");
            }
        }
        DirectoryStack { dirs }
    }
}
