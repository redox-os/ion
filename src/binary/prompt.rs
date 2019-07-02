use super::InteractiveShell;
use ion_shell::expansion::Expander;

impl<'a> InteractiveShell<'a> {
    /// Generates the prompt that will be used by Liner.
    pub fn prompt(&self) -> String {
        let shell = self.shell.borrow();
        let blocks = if self.terminated.get() { shell.block_len() } else { shell.block_len() + 1 };

        if blocks == 0 {
            shell.command("PROMPT").map(|res| res.to_string()).unwrap_or_else(|_| {
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
}
