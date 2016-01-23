use std::env::{set_current_dir, current_dir};
use std::path::PathBuf;

pub struct DirectoryStack {
    dirs: Vec<PathBuf>, // The top is always the current directory
}

impl DirectoryStack {
    pub fn new() -> Result<DirectoryStack, &'static str> {
        if let Ok(curr_dir) = current_dir() {
            Ok(DirectoryStack { dirs: vec![curr_dir] })
        } else {
            Err("Failed to get current directory when building directory stack")
        }
    }

    pub fn popd(&mut self, _: &[String]) {
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
        self.dirs.pop();
        self.print_dirs();
    }

    pub fn pushd(&mut self, args: &[String]) {
        if let Some(dir) = args.get(1) {
            match (set_current_dir(dir), current_dir()) {
                (Ok(()), Ok(cur_dir)) => {
                    self.dirs.push(cur_dir);
                }
                (Err(err), _) => {
                    println!("{}: {}", err, dir);
                    return;
                }
                (_, _) => (),
            }
        } else {
            println!("No directory provided");
            return;
        }
        self.print_dirs();
    }

    pub fn dirs(&self, _: &[String]) {
        self.print_dirs()
    }

    fn print_dirs(&self) {
        let dir = self.dirs.iter().rev().fold(String::new(), |acc, dir| acc + " " + dir.to_str().unwrap_or("No directory found"));
        println!("{}", dir.trim_left());
    }
}
