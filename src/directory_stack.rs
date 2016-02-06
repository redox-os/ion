use std::collections::VecDeque;
use std::env::{set_current_dir, current_dir};
use std::path::PathBuf;

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

    pub fn popd<I: IntoIterator>(&mut self, _: I)
        where I::Item: AsRef<str>
    {
        if self.dirs.len() < 2 {
            println!("Directory stack is empty");
            return;
        }
        if let Some(dir) = self.dirs.get(self.dirs.len() - 2) {
            if let Err(err) = set_current_dir(dir) {
                println!("{}: Failed to switch to directory {}", err, dir.display());
                return;
            }
        }
        self.dirs.pop_back();
        self.print_dirs();
    }

    pub fn pushd<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        self.change_and_push_dir(args);
        self.print_dirs();
    }

    pub fn cd<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        self.change_and_push_dir(args);
    }

    // TODO the signature for this function doesn't make a lot of sense I did
    // it this way to for ease of use where it is used, however, it should take
    // just one dir instead of args once we add features like `cd -`.
    pub fn change_and_push_dir<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        if let Some(dir) = args.into_iter().skip(1).next() {
            match (set_current_dir(dir.as_ref()), current_dir()) {
                (Ok(()), Ok(cur_dir)) => {
                    self.push_dir(cur_dir);
                }
                (Err(err), _) => {
                    println!("Failed to set current dir to {}: {}", dir.as_ref(), err);
                    return;
                }
                (_, _) => (),
            }
        } else {
            println!("No directory provided");
            return;
        }
    }

    fn push_dir(&mut self, path: PathBuf) {
        self.dirs.push_front(path);
        self.dirs.truncate(self.max_size);
    }

    pub fn dirs<I: IntoIterator>(&self, _: I)
        where I::Item: AsRef<str>
    {
        self.print_dirs()
    }

    fn print_dirs(&self) {
        let dir = self.dirs.iter().fold(String::new(), |acc, dir| {
            acc + " " + dir.to_str().unwrap_or("No directory found")
        });
        println!("{}", dir.trim_left());
    }
}
