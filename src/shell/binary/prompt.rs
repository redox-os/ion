use super::super::{Function, Shell};
use parser::shell_expand::expand_string;

pub(crate) fn prompt(shell: &mut Shell) -> String {
    if shell.flow_control.level == 0 {
        let rprompt = match prompt_fn(shell) {
            Some(prompt) => prompt,
            None => shell.variables.get_var_or_empty("PROMPT"),
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

    shell.fork_and_output(|child| unsafe {
        let _ = function.read().execute(child, &["ion"]);
    })
}
