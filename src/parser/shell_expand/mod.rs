extern crate permutate;

mod braces;
mod process;
mod ranges;
mod words;

use self::braces::BraceToken;
use self::process::{CommandExpander, CommandToken};
use self::ranges::parse_range;
use self::words::{WordIterator, WordToken};

pub struct ExpanderFunctions<'f> {
    pub tilde:    &'f Fn(&str) -> Option<String>,
    pub variable: &'f Fn(&str, bool) -> Option<String>,
    pub command:  &'f Fn(&str, bool) -> Option<String>
}

fn expand_process(current: &mut String, command: &str, quoted: bool,
    expand_func: &ExpanderFunctions)
{
    let mut expanded = String::with_capacity(command.len());
    for token in CommandExpander::new(command) {
        match token {
            CommandToken::Normal(string) => expanded.push_str(string),
            CommandToken::Variable(var) => {
                if let Some(result) = (expand_func.variable)(var, quoted) {
                    expanded.push_str(&result);
                }
            }
        }
    }

    if let Some(result) = (expand_func.command)(&expanded, quoted) {
        current.push_str(&result);
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

/// Performs shell expansions to an input string, efficiently returning the final expanded form.
/// Shells must provide their own batteries for expanding tilde and variable words.
pub fn expand_string(original: &str, expand_func: &ExpanderFunctions, reverse_quoting: bool) -> Vec<String> {
    let mut expanded_words = Vec::new();
    let mut output = String::new();
    let mut token_buffer = Vec::new();
    let mut contains_brace = false;

    for word in WordIterator::new(original) {
        if let WordToken::Brace(_) = word { contains_brace = true; }
        token_buffer.push(word);
    }

    if !token_buffer.is_empty() {
        if contains_brace {
            let mut tokens: Vec<BraceToken> = Vec::new();
            let mut expanders: Vec<Vec<String>> = Vec::new();
            let mut current = String::new();

            for word in token_buffer.drain(..) {
                match word {
                    WordToken::Brace(nodes) =>
                        expand_brace(&mut current, &mut expanders, &mut tokens, nodes, expand_func, reverse_quoting),
                    WordToken::Normal(text) => current.push_str(text),
                    WordToken::Whitespace(_) => unreachable!(),
                    WordToken::Tilde(text) => current.push_str(match (expand_func.tilde)(text) {
                        Some(ref expanded) => expanded,
                        None               => text,
                    }),
                    WordToken::Process(command, quoted) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        expand_process(&mut current, command, quoted, expand_func);
                    },
                    WordToken::Variable(text, quoted) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        current.push_str(match (expand_func.variable)(text, quoted) {
                            Some(ref var) => var,
                            None          => ""
                        });
                    },
                }
            }

            if expanders.is_empty() {
                expanded_words.push(current);
            } else {
                if !current.is_empty() {
                    tokens.push(BraceToken::Normal(current));
                }
                for word in braces::expand_braces(&tokens, expanders) {
                    expanded_words.push(word);
                }
            }
        } else {
            for word in token_buffer.drain(..) {
                match word {
                    WordToken::Brace(_) => unreachable!(),
                    WordToken::Normal(text) | WordToken::Whitespace(text) => {
                        output.push_str(text);
                    },
                    WordToken::Tilde(text) => output.push_str(match (expand_func.tilde)(text) {
                        Some(ref expanded) => expanded,
                        None               => text,
                    }),
                    WordToken::Process(command, quoted) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        expand_process(&mut output, command, quoted, expand_func);
                    },
                    WordToken::Variable(text, quoted) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        output.push_str(match (expand_func.variable)(text, quoted) {
                            Some(ref var) => var,
                            None          => ""
                        });
                    },
                }
            }

            expanded_words.push(output);
        }
    }

    expanded_words
}

// TODO: Fix these tests and write more
// TODO: Write Nested Brace Tests
//
// #[test]
// fn expand_variable_normal_variable() {
//     let input = "$A:NOT:$B";
//     let expected = "FOO:NOT:BAR";
//     let expanded = expand_string(input, |_| None, |var, _| {
//         if var == "A" { Some("FOO".to_owned()) } else if var == "B" { Some("BAR".to_owned()) } else { None }
//     }, |_, _| None).unwrap();
//     assert_eq!(expected, &expanded);
// }
//
// #[test]
// fn expand_long_braces() {
//     let line = "The pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
//     let expected = "The prodigal programmer processed prototype procedures proficiently proving prospective projections";
//     let expanded = expand_string(line, |_| None, |_, _| None, |_, _| None).unwrap();
//     assert_eq!(expected, &expanded);
// }
//
// #[test]
// fn expand_several_braces() {
//     let line = "The {barb,veget}arian eat{ers,ing} appl{esauce,ied} am{ple,ounts} of eff{ort,ectively}";
//     let expected = "The barbarian vegetarian eaters eating applesauce applied ample amounts of effort effectively";
//     let expanded = expand_string(line, |_| None, |_, _| None, |_, _| None).unwrap();
//     assert_eq!(expected, &expanded);
// }
//
// #[test]
// fn expand_several_variables() {
//     let expand_var = |var: &str, _| match var {
//         "FOO" => Some("BAR".to_owned()),
//         "X"   => Some("Y".to_owned()),
//         _     => None,
//     };
//     let expanded = expand_string("variables: $FOO $X", |_| None, expand_var, |_, _| None).unwrap();
//     assert_eq!("variables: BAR Y", &expanded);
// }
//
// #[test]
// fn expand_variable_braces() {
//     let expand_var = |var: &str, _| if var == "FOO" { Some("BAR".to_owned()) } else { None };
//     let expanded = expand_string("FOO$FOO", |_| None, expand_var, |_, _| None).unwrap();
//     assert_eq!("FOOBAR", &expanded);
//
//     let expand_var = |var: &str, _| if var == "FOO" { Some("BAR".to_owned()) } else { None };
//     let expanded = expand_string(" FOO$FOO ", |_| None, expand_var, |_, _| None).unwrap();
//     assert_eq!(" FOOBAR ", &expanded);
// }
//
// #[test]
// fn expand_variables_with_colons() {
//     let expand_var = |var: &str, _| match var {
//         "FOO" => Some("FOO".to_owned()),
//         "BAR" => Some("BAR".to_owned()),
//         _     => None,
//     };
//     let expanded = expand_string("$FOO:$BAR", |_| None, expand_var, |_, _| None).unwrap();
//     assert_eq!("FOO:BAR", &expanded);
// }
//
// #[test]
// fn expand_multiple_variables() {
//     let expand_var = |var: &str, _| match var {
//         "A" => Some("test".to_owned()),
//         "B" => Some("ing".to_owned()),
//         "C" => Some("1 2 3".to_owned()),
//         _   => None,
//     };
//     let expanded = expand_string("${A}${B}...${C}", |_| None, expand_var, |_, _| None).unwrap();
//     assert_eq!("testing...1 2 3", &expanded);
// }
//
// #[test]
// fn escape_with_backslash() {
//     let expanded = expand_string("\\$FOO\\$FOO \\$FOO", |_| None, |_, _| None, |_, _| None).unwrap();
//     assert_eq!("$FOO$FOO $FOO", &expanded);
// }
//
// #[test]
// fn expand_variable_alongside_braces() {
//     let line = "$A{1,2}";
//     let expected = "11 12";
//     let expanded = expand_string(line, |_| None, |variable, _| {
//         if variable == "A" { Some("1".to_owned()) } else { None }
//     }, |_, _| None).unwrap();
//     assert_eq!(expected, &expanded);
// }

#[test]
fn expand_variable_within_braces() {
    let line = "1{$A,2}";
    let expected = vec!["11".to_owned(), "12".to_owned()];
    let functions = ExpanderFunctions {
        tilde:    &|_| None,
        variable: &|variable: &str, _| if variable == "A" { Some("1".to_owned()) } else { None },
        command:  &|_, _| None
    };
    let expanded = expand_string(line, &functions, false);
    assert_eq!(&expected, &expanded);
}
