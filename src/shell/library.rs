use super::{Binary, FlowLogic, Shell};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub trait IonLibrary {
    /// Executes the given command and returns the exit status.
    fn execute_command(&mut self, command: &str) -> i32;
    /// Executes all of the statements contained within a given script,
    /// returning the final exit status.
    fn execute_script<P: AsRef<Path>>(&mut self, path: P) -> io::Result<i32>;
}

impl<'a> IonLibrary for Shell<'a> {
    fn execute_command(&mut self, command: &str) -> i32 {
        self.on_command(command);
        self.previous_status
    }

    fn execute_script<P: AsRef<Path>>(&mut self, path: P) -> io::Result<i32> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let capacity = file.metadata().ok().map_or(0, |x| x.len());
        let mut command_list = String::with_capacity(capacity as usize);
        let _ = file.read_to_string(&mut command_list)?;
        self.terminate_script_quotes(command_list.lines().map(|x| x.to_owned()));
        Ok(self.previous_status)
    }
}
