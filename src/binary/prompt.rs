use super::InteractiveShell;
use ion_shell::expansion::Expander;

impl<'a> InteractiveShell<'a> {
    /// Generates the prompt that will be used by Liner.
    pub fn prompt(&self) -> String {
        let shell = self.shell.borrow();
        let blocks = shell.block_len() + if shell.unterminated { 1 } else { 0 };

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
