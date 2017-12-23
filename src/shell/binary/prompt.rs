use super::super::{Capture, Function, Shell};
use parser::shell_expand::expand_string;
use std::io::Read;
use std::process;
use sys;

pub(crate) fn prompt(shell: &mut Shell) -> String {
    if shell.flow_control.level == 0 {
        match prompt_fn(shell) {
            Some(prompt) => prompt,
            None => expand_string(&shell.get_var_or_empty("PROMPT"), shell, false).join(" "),
        }
    } else {
        "    ".repeat(shell.flow_control.level as usize)
    }
}

pub(crate) fn prompt_fn(shell: &mut Shell) -> Option<String> {
    let function = shell.functions.get("PROMPT")? as *const Function;

    let mut output = None;

    match shell.fork(Capture::StdoutThenIgnoreStderr, |child| unsafe {
        let _ = function.read().execute(child, &["ion"]);
    }) {
        Ok(result) => {
            let mut string = String::new();
            match result.stdout.unwrap().read_to_string(&mut string) {
                Ok(_) => output = Some(string),
                Err(why) => {
                    eprintln!("ion: error reading stdout of child: {}", why);
                }
            }
        }
        Err(why) => {
            eprintln!("ion: fork error: {}", why);
        }
    }

    // Ensure that the parent retains ownership of the terminal before exiting.
    let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
    output
}
