use super::InteractiveShell;
use ion_shell::expansion::Expander;

impl<'a> InteractiveShell<'a> {
    /// Generates the prompt that will be used by Liner.
    pub fn prompt(&self) -> String {
        let mut shell = self.shell.borrow_mut();
        let blocks = if self.terminated.get() { shell.block_len() } else { shell.block_len() + 1 };

        if blocks == 0 {
            shell.command("PROMPT").map(|res| res.to_string()).unwrap_or_else(|_| {
                let prompt = shell.variables().get_str("PROMPT").unwrap_or_default();
                match shell.get_string(&prompt) {
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
