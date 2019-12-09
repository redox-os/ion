use super::InteractiveShell;
use ion_shell::{
    expansion::{self, Expander},
    IonError, PipelineError, Shell,
};
use liner::{KeyBindings::*, Prompt, ViPromptMode, ViStatus};

impl<'a> InteractiveShell<'a> {
    /// Generates the prompt that will be used by Liner.
    pub fn prompt(&self) -> Prompt {
        let mut shell = self.shell.borrow_mut();
        let previous_status = shell.previous_status();
        let blocks = if self.terminated.get() { shell.block_len() } else { shell.block_len() + 1 };

        if blocks == 0 {
            let out =
                shell.command("PROMPT", false).map(|res| res.to_string()).unwrap_or_else(|err| {
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
            let key_bindings = self.context.borrow().key_bindings;
            match key_bindings {
                Emacs => Prompt::from(out),
                Vi => {
                    let normal = vi_prompt_indicator(&mut shell, ViPromptMode::Normal);
                    let insert = vi_prompt_indicator(&mut shell, ViPromptMode::Insert);
                    Prompt {
                        prompt:    out,
                        vi_status: Some(ViStatus::new(ViPromptMode::Insert, normal, insert)),
                    }
                }
            }
        } else {
            Prompt::from("    ".repeat(blocks))
        }
    }
}

/// Returns the default indicators for a given mode in absence of a configuration.
///
/// NOTE: This should be made `const` once this functionality is stabilized:
/// https://github.com/rust-lang/rust/issues/49146
fn default_vi_indicator(mode: ViPromptMode) -> &'static str {
    match mode {
        ViPromptMode::Insert => "${c::dim}${c::greenbg}${c::black} I ${c::reset} ",
        ViPromptMode::Normal => "${c::dim}${c::light_graybg}${c::black} N ${c::reset} ",
    }
}

/// Returns the configuration variable for a given mode.
///
/// NOTE: This should be made `const` once this functionality is stabilized:
/// https://github.com/rust-lang/rust/issues/49146
fn vi_indicator_variable(mode: ViPromptMode) -> &'static str {
    match mode {
        ViPromptMode::Insert => "VI_INSERT",
        ViPromptMode::Normal => "VI_NORMAL",
    }
}

fn vi_prompt_indicator<'a>(shell: &mut Shell<'a>, mode: ViPromptMode) -> String {
    shell
        .variables()
        .get_str(vi_indicator_variable(mode))
        .or_else(|_| Ok(small::String::from(default_vi_indicator(mode))))
        .and_then(|prompt| shell.get_string(&prompt))
        .map(|expanded| expanded.to_string())
        .unwrap_or_else(|e| {
            eprintln!("{:?}", e);
            String::new()
        })
}
