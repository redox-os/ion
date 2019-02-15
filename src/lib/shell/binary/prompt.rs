use parser::shell_expand::expand_string;
use shell::{flags::UNTERMINATED, Capture, Function, Shell};
use std::{io::Read, process};
use sys;

pub(crate) fn prompt(shell: &mut Shell) -> String {
    let blocks =
        shell.flow_control.block.len() + if shell.flags & UNTERMINATED != 0 { 1 } else { 0 };

    if blocks == 0 {
        match prompt_fn(shell) {
            Some(prompt) => prompt,
            None => expand_string(&shell.get_str_or_empty("PROMPT"), shell, false).join(" "),
        }
    } else {
        "    ".repeat(blocks)
    }
}

pub(crate) fn prompt_fn(shell: &mut Shell) -> Option<String> {
    let function = shell.variables.get::<Function>("PROMPT")?;
    let function = &function as *const Function;

    let mut output = None;

    match shell.fork(Capture::StdoutThenIgnoreStderr, |child| {
        let _ = unsafe { function.read() }.execute(child, &["ion"]);
    }) {
        Ok(result) => {
            let mut string = String::with_capacity(1024);
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
