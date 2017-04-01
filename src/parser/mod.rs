use variables::Variables;
use directory_stack::DirectoryStack;

mod arguments;
pub mod assignments;
mod loops;
pub mod peg;
pub mod pipelines;
pub mod shell_expand;
mod statements;
mod quotes;

pub use self::shell_expand::{Index, IndexPosition, ExpanderFunctions};
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
        array: &|array: &str, index: Index| {
            match vars.get_array(array) {
                Some(array) => match index {
                        Index::None => None,
                        Index::All    => Some(array.to_owned()),
                        Index::ID(id) => array.get(id).map(|x| vec![x.to_owned()]),
                        Index::Range(start, end) => {
                            let array = match end {
                                IndexPosition::CatchAll => array.iter().skip(start)
                                    .map(|x| x.to_owned()).collect::<Vec<String>>(),
                                IndexPosition::ID(end) => array.iter().skip(start).take(end-start)
                                    .map(|x| x.to_owned()).collect::<Vec<String>>()
                            };
                            if array.is_empty() { None } else { Some(array) }
                        }
                },
                None => None
            }
        },
        variable: &|variable: &str, quoted: bool| {
            if quoted { vars.get_var(variable) } else { vars.get_var(variable).map(|x| x.replace("\n", " ")) }
        },
        command: &|command: &str, quoted: bool| vars.command_expansion(command, quoted),
    };
    shell_expand::expand_string(original, &expanders, reverse_quoting)
}
