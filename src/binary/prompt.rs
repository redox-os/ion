use ion_shell::{parser::Expander, Capture, Shell};
use std::io::Read;

pub fn prompt(shell: &Shell) -> String {
    let blocks = shell.block_len() + if shell.unterminated { 1 } else { 0 };

    if blocks == 0 {
        prompt_fn(&shell).unwrap_or_else(|| {
            shell
                .get_string(&shell.variables().get_str("PROMPT").unwrap_or_default())
                .as_str()
                .into()
        })
    } else {
        "    ".repeat(blocks)
    }
}

pub fn prompt_fn(shell: &Shell) -> Option<String> {
    shell
        .fork_function(
            Capture::StdoutThenIgnoreStderr,
            |result| {
                let mut string = String::with_capacity(1024);
                match result.stdout.ok_or(())?.read_to_string(&mut string) {
                    Ok(_) => Ok(string),
                    Err(why) => {
                        eprintln!("ion: error reading stdout of child: {}", why);
                        Err(())
                    }
                }
            },
            "PROMPT",
            &["ion"],
        )
        .ok()
}
