use super::InteractiveShell;
use ion_shell::{
    expansion::{self, Expander},
    IonError, PipelineError, Shell,
};

impl InteractiveShell {
    /// Generates the prompt that will be used by Rustyline.
    pub fn prompt(&self, shell: &mut Shell<'_>) -> String {
        let previous_status = shell.previous_status();
        let blocks = if self.terminated.get() { shell.block_len() } else { shell.block_len() + 1 };

        if blocks == 0 {
            let out = shell.command("PROMPT").map(|res| res.to_string()).unwrap_or_else(|err| {
                if let expansion::Error::Subprocess(err) = err {
                    if let IonError::PipelineExecutionError(PipelineError::CommandNotFound(_)) =
                        *err
                    {
                        match shell
                            .variables()
                            .get_str("PROMPT")
                            .and_then(|prompt| shell.get_string(&prompt))
                        {
                            Ok(prompt) => prompt.to_string(),
                            Err(err) => {
                                eprintln!("ion: prompt expansion failed: {}", err);
                                ">>> ".into()
                            }
                        }
                    } else {
                        eprintln!("ion: prompt expansion failed: {}", err);
                        ">>> ".into()
                    }
                } else {
                    panic!("Only a subprocess error should happen inside the pipeline");
                }
            });
            shell.set_previous_status(previous_status); // Set the previous exit code again
            out
        } else {
            "    ".repeat(blocks)
        }
    }
}
