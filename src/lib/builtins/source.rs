use shell::{FlowLogic, Shell};
use small;
use std::{fs::File, io::Read};

/// Evaluates the given file and returns 'SUCCESS' if it succeeds.
pub(crate) fn source(shell: &mut Shell, arguments: &[small::String]) -> Result<(), String> {
    match arguments.get(1) {
        Some(argument) => if let Ok(mut file) = File::open(argument.as_str()) {
            let capacity = file.metadata().map(|x| x.len()).unwrap_or(1) as usize;
            let mut command_list = String::with_capacity(capacity);
            file.read_to_string(&mut command_list)
                .map_err(|message| format!("ion: {}: failed to read {}\n", message, argument))
                .map(|_| {
                    for command in command_list.lines() {
                        shell.on_command(command);
                    }
                    ()
                })
        } else {
            Err(format!("ion: failed to open {}\n", argument))
        },
        None => {
            shell.evaluate_init_file();
            Ok(())
        }
    }
}
