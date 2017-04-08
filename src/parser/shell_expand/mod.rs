// TODO: Handle Runtime Errors
extern crate permutate;
extern crate unicode_segmentation;
use self::unicode_segmentation::UnicodeSegmentation;


mod braces;
mod ranges;
mod words;

use self::braces::BraceToken;
use self::ranges::parse_range;
use self::words::{WordIterator, WordToken};

pub use self::words::{Index, IndexPosition};

use std::io::{self, Write};

pub struct ExpanderFunctions<'f> {
    pub tilde:    &'f Fn(&str) -> Option<String>,
    pub array:    &'f Fn(&str, Index) -> Option<Vec<String>>,
    pub variable: &'f Fn(&str, bool) -> Option<String>,
    pub command:  &'f Fn(&str, bool) -> Option<String>
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

    let expanded = expand_tokens(tokens, expand_func, false, contains_brace).join(" ");

    if let Some(result) = (expand_func.command)(&expanded, quoted) {
        slice_string(current, &result, index);
    }
}

fn expand_brace(current: &mut String, expanders: &mut Vec<Vec<String>>,
    tokens: &mut Vec<BraceToken>, nodes: Vec<&str>, expand_func: &ExpanderFunctions,
    reverse_quoting: bool)
{
    let mut temp = Vec::new();
    for word in nodes.into_iter()
        .flat_map(|node| expand_string(node, expand_func, reverse_quoting))
    {
        match parse_range(&word) {
            Some(elements) => for word in elements { temp.push(word) },
            None           => temp.push(word),
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

fn array_expand(elements: &[&str], expand_func: &ExpanderFunctions) -> Vec<String> {
    elements.iter()
        .flat_map(|element| expand_string(element, expand_func, false))
        .collect()
}

fn array_nth(elements: &[&str], expand_func: &ExpanderFunctions, id: usize) -> String {
    elements.iter()
        .flat_map(|element| expand_string(element, expand_func, false))
        .nth(id).unwrap_or_default()
}

fn array_range(elements: &[&str], expand_func: &ExpanderFunctions, start: usize, end: IndexPosition) -> Vec<String> {
    match end {
        IndexPosition::CatchAll => elements.iter()
            .flat_map(|element| expand_string(element, expand_func, false))
            .skip(start).collect(),
        IndexPosition::ID(end) => elements.iter()
            .flat_map(|element| expand_string(element, expand_func, false))
            .skip(start).take(end-start).collect()
    }
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
        Index::Range(start, IndexPosition::ID(end)) => {
            let substring = UnicodeSegmentation::graphemes(expanded, true)
                .skip(start).take(end-start)
                .collect::<Vec<&str>>().join("");

            output.push_str(&substring);
        },
        Index::Range(start, IndexPosition::CatchAll) => {
            let substring = UnicodeSegmentation::graphemes(expanded, true)
                .skip(start).collect::<Vec<&str>>().join("");

            output.push_str(&substring);
        }
    }
}

/// Performs shell expansions to an input string, efficiently returning the final expanded form.
/// Shells must provide their own batteries for expanding tilde and variable words.
pub fn expand_string(original: &str, expand_func: &ExpanderFunctions, reverse_quoting: bool) -> Vec<String> {
    let mut token_buffer = Vec::new();
    let mut contains_brace = false;

    for word in WordIterator::new(original, true) {
        if let WordToken::Brace(_) = word { contains_brace = true; }
        token_buffer.push(word);
    }

    expand_tokens(token_buffer, expand_func, reverse_quoting, contains_brace)
}

#[allow(cyclomatic_complexity)]
fn expand_tokens(mut token_buffer: Vec<WordToken>, expand_func: &ExpanderFunctions,
    reverse_quoting: bool, contains_brace: bool) -> Vec<String>
{
    let mut output = String::new();
    let mut expanded_words = Vec::new();

    if !token_buffer.is_empty() {
        if contains_brace {
            let mut tokens: Vec<BraceToken> = Vec::new();
            let mut expanders: Vec<Vec<String>> = Vec::new();

            for word in token_buffer.drain(..) {
                match word {
                    WordToken::Array(elements, index) => {
                        match index {
                            Index::None => (),
                            Index::All => {
                                let expanded = array_expand(&elements, expand_func);
                                output.push_str(&expanded.join(" "));
                            },
                            Index::ID(id) => {
                                let expanded = array_nth(&elements, expand_func, id);
                                output.push_str(&expanded);
                            },
                            Index::Range(start, end) => {
                                let expanded = array_range(&elements, expand_func, start, end);
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
                            Index::Range(start, end) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Index::All, expand_func);
                                let temp = match end {
                                    IndexPosition::ID(end) => temp.split_whitespace()
                                        .skip(start).take(end-start)
                                        .collect::<Vec<&str>>(),
                                    IndexPosition::CatchAll => temp.split_whitespace()
                                        .skip(start).collect::<Vec<&str>>()
                                };
                                output.push_str(&temp.join(" "));
                            }
                        }
                    },
                    WordToken::ArrayMethod(array_method) => {
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
                    WordToken::Brace(nodes) =>
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
                expanded_words.push(output);
            } else {
                if !output.is_empty() {
                    tokens.push(BraceToken::Normal(output));
                }
                for word in braces::expand_braces(&tokens, expanders) {
                    expanded_words.push(word);
                }
            }

            return expanded_words
        } else if token_buffer.len() == 1 {
            match token_buffer[0].clone() {
                WordToken::Array(elements, index) => {
                    return match index {
                        Index::None   => Vec::new(),
                        Index::All    => array_expand(&elements, expand_func),
                        Index::ID(id) => vec![array_nth(&elements, expand_func, id)],
                        Index::Range(start, end) => array_range(&elements, expand_func, start, end),
                    };
                },
                WordToken::ArrayVariable(array, quoted, index) => {
                    return match (expand_func.array)(array, index) {
                        Some(ref array) if quoted => vec![array.join(" ")],
                        Some(array)               => array,
                        None                      => Vec::new(),
                    };
                },
                WordToken::ArrayProcess(command, quoted, index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    match index {
                        Index::None => return Vec::new(),
                        Index::All => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            return output.split_whitespace().map(String::from).collect::<Vec<String>>();
                        },
                        Index::ID(id) => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            return vec![output.split_whitespace().nth(id).unwrap_or_default().to_owned()];
                        }
                        Index::Range(start, end) => {
                            expand_process(&mut output, command, quoted, Index::All, expand_func);
                            return match end {
                                IndexPosition::ID(end) => output.split_whitespace().map(String::from)
                                    .skip(start).take(end-start).collect::<Vec<String>>(),
                                IndexPosition::CatchAll => output.split_whitespace().map(String::from)
                                    .skip(start).collect::<Vec<String>>()
                            }
                        },
                    }
                },
                WordToken::ArrayMethod(array_method) => {
                    return array_method.handle_as_array(expand_func);
                },
                _ => ()
            }
        }

        for word in token_buffer.drain(..) {
            match word {
                WordToken::Array(elements, index) => {
                    match index {
                        Index::None => (),
                        Index::All => {
                            let expanded = array_expand(&elements, expand_func);
                            output.push_str(&expanded.join(" "));
                        },
                        Index::ID(id) => {
                            let expanded = array_nth(&elements, expand_func, id);
                            output.push_str(&expanded);
                        },
                        Index::Range(start, end) => {
                            let expanded = array_range(&elements, expand_func, start, end);
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
                        Index::Range(start, end) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Index::All, expand_func);
                            let temp = match end {
                                IndexPosition::ID(end) => temp.split_whitespace()
                                    .skip(start).take(end-start)
                                    .collect::<Vec<&str>>(),
                                IndexPosition::CatchAll => temp.split_whitespace()
                                    .skip(start).collect::<Vec<&str>>()
                            };
                            output.push_str(&temp.join(" "));
                        },
                    }
                },
                WordToken::ArrayMethod(array_method) => {
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

        expanded_words.push(output);
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
        assert_eq!(vec![expected.to_owned()], expanded);
    }

    #[test]
    fn expand_braces() {
        let line = "pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "prodigal programmer processed prototype procedures proficiently proving prospective projections";
        let expanded = expand_string(line, &functions!(), false);
        assert_eq!(expected.split_whitespace().map(|x| x.to_owned()).collect::<Vec<String>>(), expanded);
    }

    #[test]
    fn expand_variables_with_colons() {
        let expanded = expand_string("$FOO:$BAR", &functions!(), false);
        assert_eq!(vec!["FOO:BAR".to_owned()], expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let expanded = expand_string("${B}${C}...${D}", &functions!(), false);
        assert_eq!(vec!["testing...1 2 3".to_owned()], expanded);
    }

    #[test]
    fn expand_variable_alongside_braces() {
        let line = "$A{1,2}";
        let expected = vec!["11".to_owned(), "12".to_owned()];
        let expanded = expand_string(line, &functions!(), false);
        assert_eq!(expected, expanded);
    }

    #[test]
    fn expand_variable_within_braces() {
        let line = "1{$A,2}";
        let expected = vec!["11".to_owned(), "12".to_owned()];
        let expanded = expand_string(line, &functions!(), false);
        assert_eq!(&expected, &expanded);
    }
}
