use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use variables::Variables;
use super::status::SUCCESS;

pub struct History {
    history:             VecDeque<String>,
    pub previous_status: i32,
}

impl History {
    pub fn new() -> History {
        History {
            history:         VecDeque::with_capacity(1000),
            previous_status: SUCCESS,
        }
    }

    /// Add a command to the history buffer and remove the oldest commands when the max history
    /// size has been met.
    pub fn add(&mut self, command: String, variables: &Variables) {
        // Write this command to the history file if writing to the file is enabled.
        // TODO: Prevent evaluated files from writing to the log.
        if variables.expand_string("$HISTORY_FILE_ENABLED") == "1" {
            let history_file = variables.expand_string("$HISTORY_FILE");
            History::write_to_disk(history_file, &command);
        }

        self.history.truncate(History::get_size(variables) - 1); // Make room for new item
        self.history.push_front(command);
    }

    /// If writing to the disk is enabled, this function will be used for logging history to the
    /// designated history file. If the history file does not exist, it will be created.
    fn write_to_disk(history_file: &str, command: &str) {
        match OpenOptions::new().append(true).create(true).open(history_file) {
            Ok(mut file) => {
                if let Err(message) = file.write_all(command.as_bytes()) {
                    println!("{}", message);
                }
                if let Err(message) = file.write(b"\n") {
                    println!("{}", message);
                }
                // TODO: Limit the size of the history file.
            },
            Err(message) => println!("{}", message)
        }
    }

    /// Print the entire history list currently buffered to stdout directly.
    pub fn history<I: IntoIterator>(&self, _: I) -> i32
        where I::Item: AsRef<str>
    {
        for command in self.history.iter().rev() {
            println!("{}", command);
        }
        SUCCESS
    }

    /// This function will take a map of variables as input and attempt to parse the value of the
    /// history size variable. If it succeeds, it will return the value of that variable, else it
    /// will return a default value of 1000.
    fn get_size(variables: &Variables) -> usize {
        match variables.expand_string("$HISTORY_SIZE").parse::<usize>() {
            Ok(size) => size,
            _        => 1000,
        }
    }
}
