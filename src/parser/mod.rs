#[macro_export]
macro_rules! get_expanders {
    ($vars:expr, $dir_stack:expr) => {
        ExpanderFunctions {
            vars: $vars,
            tilde: &|tilde: &str| $vars.tilde_expansion(tilde, $dir_stack),
            array: &|array: &str, selection : Select| {
                use std::iter::FromIterator;
                use $crate::types::*;
                let mut found = match $vars.get_array(array) {
                    Some(array) => match selection {
                        Select::None  => None,
                        Select::All   => Some(array.clone()),
                        Select::Index(id) => {
                            id.resolve(array.len())
                              .and_then(|n| array.get(n))
                              .map(|x| Array::from_iter(Some(x.to_owned())))
                        },
                        Select::Range(range) => {
                            if let Some((start, length)) = range.bounds(array.len()) {
                                let array = array.iter()
                                     .skip(start)
                                     .take(length)
                                     .map(|x| x.to_owned())
                                     .collect::<Array>();
                                if array.is_empty() {
                                    None
                                } else {
                                    Some(array)
                                }
                            } else {
                                None
                            }
                        },
                        Select::Key(_) => {
                            None
                        }
                    },
                    None => None
                };
                if found.is_none() {
                    found = match $vars.get_map(array) {
                        Some(map) => match selection {
                            Select::All => {
                                let mut arr = Array::new();
                                for (_, value) in map {
                                    arr.push(value.clone());
                                }
                                Some(arr)
                            }
                            Select::Key(ref key) => {
                                Some(array![
                                    map.get(key.get()).unwrap_or(&"".into()).clone()
                                ])
                            },
                            _ => None
                        },
                        None => None
                    }
                }
                found
            },
            variable: &|variable: &str, quoted: bool| {
                use ascii_helpers::AsciiReplace;
                if quoted {
                    $vars.get_var(variable)
                } else {
                    $vars.get_var(variable)
                        .map(|x| x.ascii_replace('\n', ' ').into())
                }
            },
            command: &|command: &str| $vars.command_expansion(command),
        }
    }
}

mod arguments;
pub mod assignments;
mod loops;
pub mod pipelines;
pub mod shell_expand;
mod statement;
mod quotes;

pub use self::shell_expand::{Select, Range, Index, ExpanderFunctions, expand_string, expand_tokens, WordToken, WordIterator};
pub use self::arguments::ArgumentSplitter;
pub use self::loops::for_grammar::ForExpression;
pub use self::statement::{StatementSplitter, StatementError, parse_and_validate};
pub use self::quotes::QuoteTerminator;
