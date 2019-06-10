use crate::shell::Shell;
use small;
use std::fs::File;

/// Evaluates the given file and returns 'SUCCESS' if it succeeds.
pub fn source(shell: &mut Shell<'_>, arguments: &[small::String]) -> Result<(), String> {
    match arguments.get(1) {
        Some(argument) => {
            if let Ok(file) = File::open(argument.as_str()) {
                shell.execute_command(file).map_err(|why| format!("ion: {}", why)).map(|_| ())
            } else {
                Err(format!("ion: failed to open {}\n", argument))
            }
        }
        None => {
            shell.evaluate_init_file();
            Ok(())
        }
    }
}
