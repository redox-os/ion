use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use variables::Variables;
use super::status::SUCCESS;

pub struct History {
    history:             VecDeque<String>,
    pub previous_status: i32,
}

impl Default for History {
    fn default() -> History {
        History {
            history:         VecDeque::with_capacity(1000),
            previous_status: SUCCESS,
        }
    }
}

impl History {
    /// Add a command to the history buffer and remove the oldest commands when the max history
    /// size has been met.
    pub fn add(&mut self, command: String, variables: &Variables) {
        // Write this command to the history file if writing to the file is enabled.
        // TODO: Prevent evaluated files from writing to the log.
        if variables.expand_string("$HISTORY_FILE_ENABLED") == "1" && command.trim() != "" {
            let history_file = variables.expand_string("$HISTORY_FILE");
            History::write_to_disk(&history_file, History::get_file_size(variables), &command);
        }

        self.history.truncate(History::get_size(variables) - 1); // Make room for new item
        self.history.push_front(command);
    }

    /// If writing to the disk is enabled, this function will be used for logging history to the
    /// designated history file. If the history file does not exist, it will be created.
    fn write_to_disk(history_file: &str, max_size: usize, command: &str) {
        match OpenOptions::new().read(true).write(true).create(true).open(history_file) {
            Ok(mut file) => {
                // Determine the number of commands stored and the file length.
                let (file_length, commands_stored) = {
                    let mut commands_stored = 0;
                    let mut file_length = 0;
                    let file = File::open(history_file).unwrap();
                    for byte in file.bytes() {
                        if byte.unwrap_or(b' ') == b'\n' { commands_stored += 1; }
                        file_length += 1;
                    }
                    (file_length, commands_stored)
                };

                // If the max history file size has been reached, truncate the file so that only
                // N amount of commands are listed. To truncate the file, the seek point will be
                // discovered by counting the number of bytes until N newlines have been found and
                // then the file will be seeked to that point, copying all data after and rewriting
                // the file with the first N lines removed.
                if commands_stored >= max_size {
                    let seek_point = {
                        let commands_to_delete = commands_stored - max_size + 1;
                        let mut matched = 0;
                        let mut bytes = 0;
                        let file = File::open(history_file).unwrap();
                        for byte in file.bytes() {
                            if byte.unwrap_or(b' ') == b'\n' { matched += 1; }
                            bytes += 1;
                            if matched == commands_to_delete { break }
                        }
                        bytes as u64
                    };

                    if let Err(message) = file.seek(SeekFrom::Start(seek_point)) {
                        println!("ion: unable to seek in history file: {}", message);
                    }

                    let mut buffer: Vec<u8> = Vec::with_capacity(file_length - seek_point as usize);
                    if let Err(message) = file.read_to_end(&mut buffer) {
                        println!("ion: unable to buffer history file: {}", message);
                    }

                    if let Err(message) = file.set_len(0) {
                        println!("ion: unable to truncate history file: {}", message);
                    }

                    if let Err(message) = io::copy(&mut buffer.as_slice(), &mut file) {
                        println!("ion: unable to write to history file: {}", message);
                    }
                }

                // Seek to end for appending
                if let Err(message) = file.seek(SeekFrom::End(0)) {
                    println!("ion: unable to seek in history file: {}", message);
                }

                // Write the command to the history file.
                if let Err(message) = file.write_all(command.as_bytes()) {
                    println!("ion: unable to write to history file: {}", message);
                }
                if let Err(message) = file.write(b"\n") {
                    println!("ion: unable to write to history file: {}", message);
                }
            },
            Err(message) => println!("ion: error opening file: {}", message)
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
    #[inline]
    fn get_size(variables: &Variables) -> usize {
        match variables.expand_string("$HISTORY_SIZE").parse::<usize>() {
            Ok(size) => size,
            _        => 1000,
        }
    }

    /// This function will take a map of variables as input and attempt to parse the value of the
    /// history file size variable. If it succeeds, it will return the value of that variable, else
    /// it will return a default value of 1000.
    #[inline]
    fn get_file_size(variables: &Variables) -> usize {
        match variables.expand_string("$HISTORY_FILE_SIZE").parse::<usize>() {
            Ok(size)  => size,
            Err(_)    => 1000,
        }
    }
}
