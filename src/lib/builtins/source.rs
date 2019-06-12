use crate::shell::{status::Status, Shell};
use small;
use std::fs::File;

/// Evaluates the given file and returns 'SUCCESS' if it succeeds.
pub fn source(shell: &mut Shell<'_>, arguments: &[small::String]) -> Status {
    match arguments.get(1) {
        Some(argument) => {
            if let Ok(file) = File::open(argument.as_str()) {
                if let Err(why) = shell.execute_command(file) {
                    Status::error(format!("ion: {}", why))
                } else {
                    Status::SUCCESS
                }
            } else {
                Status::error(format!("ion: failed to open {}\n", argument))
            }
        }
        None => Status::error("an argument is required for source"),
    }
}
