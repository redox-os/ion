use variables::Variables;
use directory_stack::DirectoryStack;

mod arguments;
mod loops;
pub mod peg;
pub mod pipelines;
pub mod shell_expand;
mod statements;
mod quotes;
use self::shell_expand::ExpanderFunctions;

pub use self::arguments::ArgumentSplitter;
pub use self::loops::for_grammar::ForExpression;
pub use self::statements::{StatementSplitter, StatementError, check_statement};
pub use self::quotes::QuoteTerminator;

/// Takes an argument string as input and expands it.
pub fn expand_string<'a>(original: &'a str, vars: &Variables, dir_stack: &DirectoryStack,
    reverse_quoting: bool) -> Vec<String>
{
    let expanders = ExpanderFunctions {
        tilde: &|tilde: &str| vars.tilde_expansion(tilde, dir_stack),
        variable: &|variable: &str, quoted: bool| {
            if quoted { vars.get_var(variable) } else { vars.get_var(variable).map(|x| x.replace("\n", " ")) }
        },
        command: &|command: &str, quoted: bool| vars.command_expansion(command, quoted),
    };
    shell_expand::expand_string(original, &expanders, reverse_quoting)
}
