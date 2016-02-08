use std::collections::VecDeque;
use std::env::{set_current_dir, current_dir, home_dir};
use std::path::PathBuf;
use super::status::{SUCCESS, FAILURE};

pub struct DirectoryStack {
    dirs: VecDeque<PathBuf>, // The top is always the current directory
    max_size: usize,
}

impl DirectoryStack {
    pub fn new() -> Result<DirectoryStack, &'static str> {
        let mut dirs: VecDeque<PathBuf> = VecDeque::new();
        if let Ok(curr_dir) = current_dir() {
            dirs.push_front(curr_dir);
            Ok(DirectoryStack {
                dirs: dirs,
                max_size: 1000, // TODO don't hardcode this size, make it configurable
            })
        } else {
            Err("Failed to get current directory when building directory stack")
        }
    }

    pub fn popd<I: IntoIterator>(&mut self, _: I) -> i32
        where I::Item: AsRef<str>
    {
        if let Some(dir) = self.get_previous_dir() {
            if let Err(err) = set_current_dir(dir) {
                println!("{}: Failed to switch to directory {}", err, dir.display());
                return FAILURE;
            }
        } else {
            println!("Directory stack is empty");
            return FAILURE;
        }
        self.dirs.pop_front();
        self.print_dirs();
        SUCCESS
    }

    pub fn pushd<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        if let Some(dir) = args.into_iter().nth(1) {
            let result = self.change_and_push_dir(dir.as_ref());
            self.print_dirs();
            result
        } else {
            println!("No directory provided");
            FAILURE
        }
    }

    pub fn cd<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        if let Some(dir) = args.into_iter().nth(1) {
            let dir = dir.as_ref();
            if dir == "-" {
                self.switch_to_previous_directory()
            } else {
                self.change_and_push_dir(dir)
            }
        } else {
            self.switch_to_home_directory()
        }
    }

    fn switch_to_home_directory(&mut self) -> i32 {
        if let Some(home) = home_dir() {
            if let Some(home) = home.to_str() {
                self.change_and_push_dir(home)
            } else {
                println!("Failed to convert home directory to str");
                FAILURE
            }
        } else {
            println!("Failed to get home directory");
            FAILURE
        }
    }

    fn switch_to_previous_directory(&mut self) -> i32 {
        if let Some(prev) = self.get_previous_dir()
                                .map(|path| path.to_string_lossy().to_string()) {
            println!("{}", prev);
            self.change_and_push_dir(&prev)
        } else {
            println!("No previous directory to switch to");
            FAILURE
        }
    }

    fn get_previous_dir(&self) -> Option<&PathBuf> {
        if self.dirs.len() < 2 {
            None
        } else {
            self.dirs.get(1)
        }
    }

    pub fn change_and_push_dir(&mut self, dir: &str) -> i32 {
        match (set_current_dir(dir), current_dir()) {
            (Ok(()), Ok(cur_dir)) => {
                self.push_dir(cur_dir);
                SUCCESS
            }
            (Err(err), _) => {
                println!("Failed to set current dir to {}: {}", dir, err);
                FAILURE
            }
            (_, _) => FAILURE // This should not happen
        }
    }

    fn push_dir(&mut self, path: PathBuf) {
        self.dirs.push_front(path);
        self.dirs.truncate(self.max_size);
    }

    pub fn dirs<I: IntoIterator>(&self, _: I) -> i32
        where I::Item: AsRef<str>
    {
        self.print_dirs();
        SUCCESS
    }

    fn print_dirs(&self) {
        let dir = self.dirs.iter().fold(String::new(), |acc, dir| {
            acc + " " + dir.to_str().unwrap_or("No directory found")
        });
        println!("{}", dir.trim_left());
    }
}
