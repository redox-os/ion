// TODO: Handle Runtime Errors
extern crate permutate;
extern crate unicode_segmentation;
use self::unicode_segmentation::UnicodeSegmentation;

use types::Array;

mod braces;
mod ranges;
mod words;

use self::braces::BraceToken;
use self::ranges::parse_range;
pub use self::words::{WordIterator, WordToken};

pub use self::words::{Index, IndexStart, IndexEnd};

use std::io::{self, Write};
use types::*;

pub struct ExpanderFunctions<'f> {
    pub tilde:    &'f Fn(&str) -> Option<String>,
    pub array:    &'f Fn(&str, Index) -> Option<Array>,
    pub variable: &'f Fn(&str, bool) -> Option<Value>,
    pub command:  &'f Fn(&str, bool) -> Option<Value>
}

fn expand_process(current: &mut String, command: &str, quoted: bool,
    index: Index, expand_func: &ExpanderFunctions)
{
    let mut tokens = Vec::new();
    let mut contains_brace = false;

    for token in WordIterator::new(command, false) {
        if let WordToken::Brace(_) = token { contains_brace = true; }
        tokens.push(token);
    }

    let expanded = expand_tokens(&tokens, expand_func, false, contains_brace).join(" ");

    if let Some(result) = (expand_func.command)(&expanded, quoted) {
        slice_string(current, &result, index);
    }
}

fn expand_brace(current: &mut String, expanders: &mut Vec<Vec<String>>,
    tokens: &mut Vec<BraceToken>, nodes: &[&str], expand_func: &ExpanderFunctions,
    reverse_quoting: bool)
{
    let mut temp = Vec::new();
    for word in nodes.into_iter()
        .flat_map(|node| expand_string(node, expand_func, reverse_quoting))
    {
        match parse_range(&word) {
            Some(elements) => for word in elements { temp.push(word.into()) },
            None           => temp.push(word.into()),
        }
    }

    if !temp.is_empty() {
        if !current.is_empty() {
            tokens.push(BraceToken::Normal(current.clone()));
            current.clear();
        }
        tokens.push(BraceToken::Expander);
        expanders.push(temp);
    } else {
        current.push_str("{}");
    }
}

fn array_expand(elements: &[&str], expand_func: &ExpanderFunctions) -> Array {
    elements.iter()
        .flat_map(|element| expand_string(element, expand_func, false))
        .collect()
}

fn array_nth(elements: &[&str], expand_func: &ExpanderFunctions, id: usize) -> Value {
    elements.iter()
        .flat_map(|element| expand_string(element, expand_func, false))
        .nth(id).unwrap_or_default()
}

fn array_range(elements: &[&str], expand_func: &ExpanderFunctions, start: IndexStart, end: IndexEnd) -> Array {
    let len = elements.len();
    elements.iter()
            .flat_map(|element| expand_string(element, expand_func, false))
            .skip(start.resolve(len))
            .take(end.diff(&start, len))
            .collect()
}

fn slice_string(output: &mut String, expanded: &str, index: Index) {
    match index {
        Index::None => (),
        Index::All => output.push_str(expanded),
        Index::ID(id) => {
            if let Some(character) = UnicodeSegmentation::graphemes(expanded, true).nth(id) {
                output.push_str(character);
            }
        },
        Index::FromEnd(id) => {
            let mut graphemes = UnicodeSegmentation::graphemes(expanded, true);
            let len = graphemes.clone().count();
            if let Some(character) = graphemes.nth(len - id) {
                output.push_str(character);
            }
        }
        Index::Range(start, end) => {
            let graphemes = UnicodeSegmentation::graphemes(expanded, true);
            let len = graphemes.clone().count();
            let substring = graphemes.skip(start.resolve(len))
                                     .take(end.diff(&start, len))
                                     .collect::<Vec<&str>>()
                                     .join("");

            output.push_str(&substring);
        },
    }
}

/// Performs shell expansions to an input string, efficiently returning the final expanded form.
/// Shells must provide their own batteries for expanding tilde and variable words.
pub fn expand_string(
    original: &str,
    expand_func: &ExpanderFunctions,
    reverse_quoting: bool
) -> Array {
    let mut token_buffer = Vec::new();
    let mut contains_brace = false;

    for word in WordIterator::new(original, true) {
        if let WordToken::Brace(_) = word { contains_brace = true; }
        token_buffer.push(word);
    }

    expand_tokens(
        &token_buffer,
        expand_func,
        reverse_quoting,
        contains_brace
    )
}

#[allow(cyclomatic_complexity)]
pub fn expand_tokens<'a>(token_buffer: &[WordToken], expand_func: &'a ExpanderFunctions,
    reverse_quoting: bool, contains_brace: bool) -> Array
{
    let mut output = String::new();
    let mut expanded_words = Array::new();

    if !token_buffer.is_empty() {
        if contains_brace {
            let mut tokens: Vec<BraceToken> = Vec::new();
            let mut expanders: Vec<Vec<String>> = Vec::new();

            for word in token_buffer {
                match *word {
                    WordToken::Array(ref elements, index) => {
                        match index {
                            Index::None => (),
                            Index::All => {
                                let expanded = array_expand(elements, expand_func);
                                output.push_str(&expanded.join(" "));
                            },
                            Index::ID(id) => {
                                let expanded = array_nth(elements, expand_func, id);
                                output.push_str(&expanded);
                            },
                            Index::FromEnd(id) => {
                                let expanded = array_nth(elements,
                                                         expand_func,
                                                         elements.len() - id);
                                output.push_str(&expanded);
                            }
                            Index::Range(start, end) => {
                                let expanded = array_range(elements, expand_func, start, end);
                                output.push_str(&expanded.join(" "));
                            }
                        };
                    },
                    WordToken::ArrayVariable(array, _, index) => {
                        if let Some(array) = (expand_func.array)(array, index) {
                            output.push_str(&array.join(" "));
                        }
                    },
                    WordToken::ArrayProcess(command, quoted, index) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        match index {
                            Index::None => (),
                            Index::All => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Index::All, expand_func);
                                let temp = temp.split_whitespace().collect::<Vec<&str>>();
                                output.push_str(&temp.join(" "));
                            },
                            Index::ID(id) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Index::All, expand_func);
                                output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                            },
                            Index::FromEnd(id) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Index::All, expand_func);
                                output.push_str(temp.split_whitespace()
                                                    .rev()
                                                    .nth(id - 1)
                                                    .unwrap_or_default());
                            }
                            Index::Range(start, end) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Index::All, expand_func);
                                let len = temp.split_whitespace().count();
                                let temp = temp.split_whitespace()
                                               .skip(start.resolve(len))
                                               .take(end.diff(&start, len))
                                               .collect::<Vec<&str>>();
                                output.push_str(&temp.join(" "));
                            }
                        }
                    },
                    WordToken::ArrayMethod(ref array_method) => {
                        array_method.handle(&mut output, expand_func);
                    },
                    WordToken::StringMethod(method, variable, pattern, index) => {
                        let pattern = &expand_string(pattern, expand_func, false).join(" ");
                        match method {
                            "join" => if let Some(array) = (expand_func.array)(variable, Index::All) {
                                slice_string(&mut output, &array.join(pattern), index);
                            },
                            _ => {
                                let stderr = io::stderr();
                                let mut stderr = stderr.lock();
                                let _ = writeln!(stderr, "ion: invalid string method: {}", method);
                            }
                        }
                    },
                    WordToken::Brace(ref nodes) =>
                        expand_brace(&mut output, &mut expanders, &mut tokens, nodes, expand_func, reverse_quoting),
                    WordToken::Normal(text) => output.push_str(text),
                    WordToken::Whitespace(_) => unreachable!(),
                    WordToken::Tilde(text) => output.push_str(match (expand_func.tilde)(text) {
                        Some(ref expanded) => expanded,
                        None               => text,
                    }),
                    WordToken::Process(command, quoted, index) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        expand_process(&mut output, command, quoted, index, expand_func);
                    },
                    WordToken::Variable(text, quoted, index) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        let expanded = match (expand_func.variable)(text, quoted) {
                            Some(var) => var,
                            None      => continue
                        };

                        slice_string(&mut output, &expanded, index);
                    },
                }
            }

            if expanders.is_empty() {
                expanded_words.push(output.into());
            } else {
                if !output.is_empty() {
                    tokens.push(BraceToken::Normal(output));
                }
                for word in braces::expand_braces(&tokens, expanders) {
                    expanded_words.push(word.into());
                }
            }

            return expanded_words;
        } else if token_buffer.len() == 1 {
            match token_buffer[0] {
                WordToken::Array(ref elements, index) => {
                    return match index {
                        Index::None   => Array::new(),
                        Index::All    => array_expand(elements, expand_func),
                        Index::ID(id) =>
                            Some(array_nth(elements, expand_func, id))
                                .into_iter().collect(),
                        Index::FromEnd(id) =>
                            Some(array_nth(elements, expand_func, elements.len() - id))
                                .into_iter()
                                .collect(),
                        Index::Range(start, end) => array_range(elements, expand_func, start, end),
                    };
                },
                WordToken::ArrayVariable(array, quoted, index) => {
                    return match (expand_func.array)(array, index) {
                        Some(ref array) if quoted =>
                            Some(array.join(" ").into()).into_iter().collect(),
                        Some(array)               => array,
                        None                      => Array::new(),
                    };
                },
                WordToken::ArrayProcess(command, quoted, index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    match index {
                        Index::None => return Array::new(),
                        Index::All => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            return output.split_whitespace()
                                .map(From::from)
                                .collect::<Array>();
                        },
                        Index::ID(id) => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            return Some(
                                output.split_whitespace().nth(id)
                                    .unwrap_or_default()
                                    .into()
                            ).into_iter()
                                .collect();
                        },
                        Index::FromEnd(id) => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            return Some(
                                output.split_whitespace()
                                      .rev()
                                      .nth(id - 1)
                                      .unwrap_or_default()
                                      .into()
                            ).into_iter()
                             .collect();
                        }
                        Index::Range(start, end) => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            let len = output.split_whitespace().count();
                            return output.split_whitespace()
                                  .skip(start.resolve(len))
                                  .take(end.diff(&start, len))
                                  .map(From::from)
                                  .collect::<Array>();
                        },
                    }
                },
                WordToken::ArrayMethod(ref array_method) => {
                    return array_method.handle_as_array(expand_func);
                },
                _ => ()
            }
        }

        for word in token_buffer {
            match *word {
                WordToken::Array(ref elements, index) => {
                    match index {
                        Index::None => (),
                        Index::All => {
                            let expanded = array_expand(elements, expand_func);
                            output.push_str(&expanded.join(" "));
                        },
                        Index::ID(id) => {
                            let expanded = array_nth(elements, expand_func, id);
                            output.push_str(&expanded);
                        },
                        Index::FromEnd(id) => {
                            let expanded = array_nth(elements, expand_func, elements.len() - id);
                            output.push_str(&expanded);
                        }
                        Index::Range(start, end) => {
                            let expanded = array_range(elements, expand_func, start, end);
                            output.push_str(&expanded.join(" "));
                        },
                    };
                },
                WordToken::ArrayVariable(array, _, index) => {
                    if let Some(array) = (expand_func.array)(array, index) {
                        output.push_str(&array.join(" "));
                    }
                },
                WordToken::ArrayProcess(command, quoted, index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    match index {
                        Index::None => (),
                        Index::All => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Index::All, expand_func);
                            let temp = temp.split_whitespace().collect::<Vec<&str>>();
                            output.push_str(&temp.join(" "));
                        },
                        Index::ID(id) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Index::All, expand_func);
                            output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                        },
                        Index::FromEnd(id) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Index::All, expand_func);
                            output.push_str(temp.split_whitespace()
                                                .rev()
                                                .nth(id - 1)
                                                .unwrap_or_default());
                        },
                        Index::Range(start, end) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Index::All, expand_func);
                            let len = temp.split_whitespace().count();
                            let temp = temp.split_whitespace()
                                           .skip(start.resolve(len))
                                           .take(end.diff(&start, len))
                                           .collect::<Vec<&str>>();
                            output.push_str(&temp.join(" "));
                        },
                    }
                },
                WordToken::ArrayMethod(ref array_method) => {
                    array_method.handle(&mut output, expand_func);
                },
                WordToken::StringMethod(method, variable, pattern, index) => {
                    let pattern = &expand_string(pattern, expand_func, false).join(" ");
                    match method {
                        "join" => if let Some(array) = (expand_func.array)(variable, Index::All) {
                            slice_string(&mut output, &array.join(pattern), index);
                        },
                        _ => {
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "ion: invalid string method: {}", method);
                        }
                    }
                },
                WordToken::Brace(_) => unreachable!(),
                WordToken::Normal(text) | WordToken::Whitespace(text) => {
                    output.push_str(text);
                },
                WordToken::Process(command, quoted, index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    expand_process(&mut output, command, quoted, index, expand_func);
                }
                WordToken::Tilde(text) => output.push_str(match (expand_func.tilde)(text) {
                    Some(ref expanded) => expanded,
                    None               => text,
                }),
                WordToken::Variable(text, quoted, index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    let expanded = match (expand_func.variable)(text, quoted) {
                        Some(var) => var,
                        None          => continue
                    };

                    slice_string(&mut output, &expanded, index);
                },
            }
        }

        expanded_words.push(output.into());
    }

    expanded_words
}

// TODO: Write Nested Brace Tests

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! functions {
        () => {
            ExpanderFunctions {
                tilde:    &|_| None,
                array:    &|_, _| None,
                variable: &|variable: &str, _| match variable {
                    "A" => Some("1".to_owned()),
                    "B" => Some("test".to_owned()),
                    "C" => Some("ing".to_owned()),
                    "D" => Some("1 2 3".to_owned()),
                    "FOO" => Some("FOO".to_owned()),
                    "BAR" => Some("BAR".to_owned()),
                    _   => None
                },
                command:  &|_, _| None
            }
        }
    }

    #[test]
    fn expand_variable_normal_variable() {
        let input = "$FOO:NOT:$BAR";
        let expected = "FOO:NOT:BAR";
        let expanded = expand_string(input, &functions!(), false);
        assert_eq!(Array::from_vec(vec![expected.to_owned()]), expanded);
    }

    #[test]
    fn expand_braces() {
        let line = "pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "prodigal programmer processed prototype procedures proficiently proving prospective projections";
        let expanded = expand_string(line, &functions!(), false);
        assert_eq!(
            expected.split_whitespace()
                .map(|x| x.to_owned())
                .collect::<Array>(),
            expanded
        );
    }

    #[test]
    fn expand_variables_with_colons() {
        let expanded = expand_string("$FOO:$BAR", &functions!(), false);
        assert_eq!(Array::from_vec(vec!["FOO:BAR".to_owned()]), expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let expanded = expand_string("${B}${C}...${D}", &functions!(), false);
        assert_eq!(Array::from_vec(vec!["testing...1 2 3".to_owned()]), expanded);
    }

    #[test]
    fn expand_variable_alongside_braces() {
        let line = "$A{1,2}";
        let expected = Array::from_vec(vec!["11".to_owned(), "12".to_owned()]);
        let expanded = expand_string(line, &functions!(), false);
        assert_eq!(expected, expanded);
    }

    #[test]
    fn expand_variable_within_braces() {
        let line = "1{$A,2}";
        let expected = Array::from_vec(vec!["11".to_owned(), "12".to_owned()]);
        let expanded = expand_string(line, &functions!(), false);
        assert_eq!(&expected, &expanded);
    }

    #[test]
    fn array_indexing() {
        let base = |idx : &str| format!("[1 2 3][{}]", idx);
        let expander = functions!();
        {
            let expected = Array::from_vec(vec!["1".to_owned()]);
            let idxs = vec!["-3", "0", "..-2"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander, false));
            }
        }
        {
            let expected = Array::from_vec(vec!["2".to_owned(), "3".to_owned()]);
            let idxs = vec!["1...2", "1...-1"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander, false));
            }
        }
    }
}
