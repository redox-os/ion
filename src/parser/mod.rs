use variables::Variables;
use directory_stack::DirectoryStack;
use parser::shell_expand::ExpandErr;

mod loops;
pub mod peg;
pub mod pipelines;
pub mod shell_expand;
mod statements;

pub use self::loops::for_grammar::ForExpression;
pub use self::statements::{StatementSplitter, StatementError};

/// Takes an argument string as input and expands it.
pub fn expand_string<'a>(original: &'a str, vars: &Variables, dir_stack: &DirectoryStack) -> Result<String, ExpandErr> {
    let tilde_fn    = |tilde:    &str| vars.tilde_expansion(tilde, dir_stack);
    let variable_fn = |variable: &str, quoted: bool| {
        if quoted { vars.get_var(variable) } else { vars.get_var(variable).map(|x| x.replace("\n", " ")) }
    };
    let command_fn  = |command:  &str, quoted: bool| vars.command_expansion(command, quoted);
    shell_expand::expand_string(original, tilde_fn, variable_fn, command_fn)
}
