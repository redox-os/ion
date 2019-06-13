use super::InteractiveBinary;
use ion_shell::{parser::Expander, Capture, Shell};
use std::io::Read;

impl<'a> InteractiveBinary<'a> {
    /// Generates the prompt that will be used by Liner.
    pub fn prompt(&self) -> String {
        let shell = self.shell.borrow();
        let blocks = shell.block_len() + if shell.unterminated { 1 } else { 0 };

        if blocks == 0 {
            Self::prompt_fn(&shell).unwrap_or_else(|| {
                match shell.get_string(&shell.variables().get_str("PROMPT").unwrap_or_default()) {
                    Ok(prompt) => prompt.to_string(),
                    Err(why) => {
                        eprintln!("ion: prompt expansion failed: {}", why);
                        ">>> ".into()
                    }
                }
            })
        } else {
            "    ".repeat(blocks)
        }
    }

    pub fn prompt_fn(shell: &Shell<'_>) -> Option<String> {
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
}
