use crate::{
    parser::shell_expand::expand_string,
    shell::{flags::UNTERMINATED, variables::Value, Capture, Shell},
    sys,
};
use std::{io::Read, process};

pub(crate) fn prompt(shell: &mut Shell) -> String {
    let blocks =
        shell.flow_control.block.len() + if shell.flags & UNTERMINATED != 0 { 1 } else { 0 };

    if blocks == 0 {
        prompt_fn(shell)
            .unwrap_or_else(|| expand_string(&shell.get_str_or_empty("PROMPT"), shell).join(" "))
    } else {
        "    ".repeat(blocks)
    }
}

pub(crate) fn prompt_fn(shell: &mut Shell) -> Option<String> {
    if let Some(Value::Function(function)) = shell.variables.get_ref("PROMPT") {
        let output = match shell.fork(Capture::StdoutThenIgnoreStderr, move |child| {
            let _ = function.execute(child, &["ion"]);
        }) {
            Ok(result) => {
                let mut string = String::with_capacity(1024);
                match result.stdout?.read_to_string(&mut string) {
                    Ok(_) => Some(string),
                    Err(why) => {
                        eprintln!("ion: error reading stdout of child: {}", why);
                        None
                    }
                }
            }
            Err(why) => {
                eprintln!("ion: fork error: {}", why);
                None
            }
        };

        // Ensure that the parent retains ownership of the terminal before exiting.
        let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
        output
    } else {
        None
    }
}
