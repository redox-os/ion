use std::borrow::Cow;
use std::collections::VecDeque;
use std::env::{set_current_dir, current_dir, home_dir};
use std::path::PathBuf;
use super::variables::Variables;
use super::status::SUCCESS;

pub struct DirectoryStack {
    dirs: VecDeque<PathBuf>, // The top is always the current directory
}

impl DirectoryStack {
    /// Create a new `DirectoryStack` containing the current working directory, if available.
    pub fn new() -> DirectoryStack {
        let mut dirs: VecDeque<PathBuf> = VecDeque::new();
        match current_dir() {
            Ok(curr_dir) => {
                dirs.push_front(curr_dir);
                DirectoryStack { dirs: dirs }
            },
            Err(_) => {
                eprintln!("ion: failed to get current directory when building directory stack");
                DirectoryStack { dirs: dirs}
            }
        }
    }

    /// This function will take a map of variables as input and attempt to parse the value of the
    /// directory stack size variable. If it succeeds, it will return the value of that variable,
    /// else it will return a default value of 1000.
    fn get_size(variables: &Variables) -> usize {
        variables.get_var_or_empty("DIRECTORY_STACK_SIZE").parse::<usize>().unwrap_or(1000)
    }

    /// Attempts to set the current directory to the directory stack's previous directory,
    /// and then removes the front directory from the stack.
    pub fn popd<I: IntoIterator>(&mut self, _: I) -> Result<(), Cow<'static, str>>
        where I::Item: AsRef<str>
    {
        self.get_previous_dir().cloned()
            .map_or(Err(Cow::Borrowed("ion: directory stack is empty\n")), |dir| {
                set_current_dir(&dir)
                    .map_err(|err| { Cow::Owned(format!("ion: {}: Failed to switch to directory {}\n", err, dir.display())) })
                    .map(|_| { self.dirs.pop_front(); self.print_dirs(); () })
            })
    }

    pub fn pushd<I: IntoIterator>(&mut self, args: I, variables: &Variables) -> Result<(), Cow<'static, str>>
        where I::Item: AsRef<str>
    {
        args.into_iter().nth(1)
            .map_or_else(|| { Err(Cow::Borrowed("ion: no directory provided\n")) }, |dir| {
                let result = self.change_and_push_dir(dir.as_ref(), variables);
                self.print_dirs();
                result
            })
    }

    pub fn cd<I: IntoIterator>(&mut self, args: I, variables: &Variables) -> Result<(), Cow<'static, str>>
        where I::Item: AsRef<str>
    {
            match args.into_iter().nth(1) {
                Some(dir) => {
                    let dir = dir.as_ref();
                    if dir == "-" {
                        self.switch_to_previous_directory(variables)
                    } else {
                        self.change_and_push_dir(dir, variables)
                    }
                }
                None => self.switch_to_home_directory(variables)
            }
    }

    fn switch_to_home_directory(&mut self, variables: &Variables) -> Result<(), Cow<'static, str>> {
        home_dir()
            .map_or(Err(Cow::Borrowed("ion: failed to get home directory")), |home| {
                home.to_str().map_or(Err(Cow::Borrowed("ion: failed to convert home directory to str")), |home| {
                    self.change_and_push_dir(home, variables)
                })
            })
    }

    fn switch_to_previous_directory(&mut self, variables: &Variables) -> Result<(), Cow<'static, str>> {
        self.get_previous_dir().cloned()
            .map_or_else(|| Err(Cow::Borrowed("ion: no previous directory to switch to")), |prev| {
                self.dirs.remove(1);
                let prev = prev.to_string_lossy().to_string();
                println!("{}", prev);
                self.change_and_push_dir(&prev, variables)
            })
    }

    fn get_previous_dir(&self) -> Option<&PathBuf> {
        if self.dirs.len() < 2 {
            None
        } else {
            self.dirs.get(1)
        }
    }

    pub fn change_and_push_dir(&mut self, dir: &str, variables: &Variables) -> Result<(), Cow<'static, str>> {
        match (set_current_dir(dir), current_dir()) {
            (Ok(()), Ok(cur_dir)) => {
                self.push_dir(cur_dir, variables);
                Ok(())
            }
            (Err(err), _) => {
                Err(Cow::Owned(format!("ion: failed to set current dir to {}: {}\n", dir, err)))
            }
            (_, _) => Err(Cow::Borrowed("ion: change_and_push_dir(): error occurred that should never happen\n")), // This should not happen
        }
    }

    fn push_dir(&mut self, path: PathBuf, variables: &Variables) {
        self.dirs.push_front(path);

        self.dirs.truncate(DirectoryStack::get_size(variables));
    }

    pub fn dirs<I: IntoIterator>(&self, _: I) -> i32
        where I::Item: AsRef<str>
    {
        self.print_dirs();
        SUCCESS
    }

    pub fn dir_from_top(&self, num: usize) -> Option<&PathBuf> {
        self.dirs.get(num)
    }

    pub fn dir_from_bottom(&self, num: usize) -> Option<&PathBuf> {
        self.dirs.iter().rev().nth(num)
    }

    fn print_dirs(&self) {
        let dir = self.dirs.iter().fold(String::new(), |acc, dir| {
            acc + " " + dir.to_str().unwrap_or("ion: no directory found")
        });
        println!("{}", dir.trim_left());
    }
}
