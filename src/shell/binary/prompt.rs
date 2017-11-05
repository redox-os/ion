use super::super::{Function, Shell};
use parser::shell_expand::expand_string;
use std::io::Read;
use std::process;
use sys;

pub(crate) fn prompt(shell: &mut Shell) -> String {
    if shell.flow_control.level == 0 {
        let rprompt = match prompt_fn(shell) {
            Some(prompt) => prompt,
            None => shell.get_var_or_empty("PROMPT"),
        };
        expand_string(&rprompt, shell, false).join(" ")
    } else {
        "    ".repeat(shell.flow_control.level as usize)
    }
}

pub(crate) fn prompt_fn(shell: &mut Shell) -> Option<String> {
    let function = match shell.functions.get("PROMPT") {
        Some(func) => func as *const Function,
        None => return None,
    };

    let mut output = None;

    match shell.fork(|child| unsafe {
        let _ = function.read().execute(child, &["ion"]);
    }) {
        Ok(mut result) => {
            let mut string = String::new();
            match result.stdout.read_to_string(&mut string) {
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
