// TODO: Handle Runtime Errors
extern crate permutate;
extern crate unicode_segmentation;
extern crate calc;
use self::unicode_segmentation::UnicodeSegmentation;

use types::Array;

mod braces;
mod ranges;
mod words;
use glob::glob;
use self::braces::BraceToken;
use self::ranges::parse_range;
pub use self::words::{WordIterator, WordToken, Select, Index, Range};
use shell::variables::Variables;

use std::io::{self, Write};
use types::*;

pub struct ExpanderFunctions<'f> {
    pub vars:     &'f Variables,
    pub tilde:    &'f Fn(&str) -> Option<String>,
    pub array:    &'f Fn(&str, Select) -> Option<Array>,
    pub variable: &'f Fn(&str, bool) -> Option<Value>,
    pub command:  &'f Fn(&str, bool) -> Option<Value>
}

fn expand_process(current: &mut String, command: &str, quoted: bool,
    selection: Select, expand_func: &ExpanderFunctions)
{
    let mut tokens = Vec::new();
    let mut contains_brace = false;

    for token in WordIterator::new(command, false, expand_func) {
        if let WordToken::Brace(_) = token { contains_brace = true; }
        tokens.push(token);
    }

    let expanded = expand_tokens(&tokens, expand_func, false, contains_brace).join(" ");

    if let Some(result) = (expand_func.command)(&expanded, quoted) {
        slice_string(current, &result, selection);
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

fn array_expand(elements : &[&str], expand_func: &ExpanderFunctions, selection : Select) -> Array {
    match selection {
        Select::None => Array::new(),
        Select::All => elements.iter().flat_map(|e| expand_string(e, expand_func, false)).collect(),
        Select::Index(index) => array_nth(elements, expand_func, index).into_iter().collect(),
        Select::Range(range) => array_range(elements, expand_func, range),
    }
}

fn array_nth(elements: &[&str], expand_func: &ExpanderFunctions, index: Index) -> Option<Value> {
    let mut expanded = elements.iter().flat_map(|e| expand_string(e, expand_func, false));
    match index {
        Index::Forward(n) => expanded.nth(n),
        Index::Backward(n) => expanded.rev().nth(n),
    }
}

fn array_range(elements: &[&str], expand_func: &ExpanderFunctions, range : Range) -> Array {
    let expanded = elements.iter()
                           .flat_map(|e| expand_string(e, expand_func, false))
                           .collect::<Array>();
    let len = expanded.len();
    if let Some((start, length)) = range.bounds(len) {
        expanded.into_iter()
                .skip(start)
                .take(length)
                .collect()
    } else {
        Array::new()
    }
}

fn slice_string(output: &mut String, expanded: &str, selection: Select) {
    match selection {
        Select::None => (),
        Select::All => output.push_str(expanded),
        Select::Index(Index::Forward(id)) => {
            if let Some(character) = UnicodeSegmentation::graphemes(expanded, true).nth(id) {
                output.push_str(character);
            }
        },
        Select::Index(Index::Backward(id)) => {
            if let Some(character) = UnicodeSegmentation::graphemes(expanded, true).rev().nth(id) {
                output.push_str(character);
            }
        }
        Select::Range(range) => {
            let graphemes = UnicodeSegmentation::graphemes(expanded, true);
            if let Some((start, length)) = range.bounds(graphemes.clone().count()) {
                let substring = graphemes.skip(start)
                                         .take(length)
                                         .collect::<Vec<&str>>()
                                         .join("");
               output.push_str(&substring);
            }
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

    for word in WordIterator::new(original, true, expand_func) {
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
    let mut is_glob = false;
    if !token_buffer.is_empty() {
        if contains_brace {
            let mut tokens: Vec<BraceToken> = Vec::new();
            let mut expanders: Vec<Vec<String>> = Vec::new();

            for word in token_buffer {
                match *word {
                    WordToken::Array(ref elements, index) => {
                        output.push_str(&array_expand(elements, expand_func, index).join(" "));
                    },
                    WordToken::ArrayVariable(array, _, index) => {
                        if let Some(array) = (expand_func.array)(array, index) {
                            output.push_str(&array.join(" "));
                        }
                    },
                    WordToken::ArrayProcess(command, quoted, index) => {
                        let quoted = if reverse_quoting { !quoted } else { quoted };
                        match index {
                            Select::None => (),
                            Select::All => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Select::All, expand_func);
                                let temp = temp.split_whitespace().collect::<Vec<&str>>();
                                output.push_str(&temp.join(" "));
                            },
                            Select::Index(Index::Forward(id)) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Select::All, expand_func);
                                output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                            },
                            Select::Index(Index::Backward(id)) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Select::All, expand_func);
                                output.push_str(temp.split_whitespace()
                                                    .rev()
                                                    .nth(id)
                                                    .unwrap_or_default());
                            }
                            Select::Range(range) => {
                                let mut temp = String::new();
                                expand_process(&mut temp, command, quoted, Select::All, expand_func);
                                let len = temp.split_whitespace().count();
                                if let Some((start, length)) = range.bounds(len) {
                                    let res = temp.split_whitespace()
                                                  .skip(start)
                                                  .take(length)
                                                  .collect::<Vec<&str>>();
                                    output.push_str(&res.join(" "));
                                }
                            }
                        }
                    },
                    WordToken::ArrayMethod(ref array_method) => {
                        array_method.handle(&mut output, expand_func);
                    },
                    WordToken::StringMethod(method, variable, pattern, index) => {
                        let pattern = &expand_string(pattern, expand_func, false).join(" ");
                        match method {
                            "join" => if let Some(array) = (expand_func.array)(variable, Select::All) {
                                slice_string(&mut output, &array.join(pattern), index);
                            },
                            "len" => output.push_str(&UnicodeSegmentation::graphemes (
                                expand_func.vars.get_var_or_empty(variable).as_str(), true
                            ).count().to_string()),
                            "len_bytes" => output.push_str(
                                &expand_func.vars.get_var_or_empty(variable).len().to_string()
                            ),
                            _ => {
                                let stderr = io::stderr();
                                let mut stderr = stderr.lock();
                                let _ = writeln!(stderr, "ion: invalid string method: {}", method);
                            }
                        }
                    },
                    WordToken::Brace(ref nodes) =>
                        expand_brace(&mut output, &mut expanders, &mut tokens, nodes, expand_func, reverse_quoting),
                    WordToken::Normal(text,false) => output.push_str(text),
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
                    WordToken::Normal(text,true) => {
                        let globbed = glob(text);
                        if let Ok(var)=globbed{
                            for path in var.filter_map(Result::ok) {
                                expanded_words.push(path.to_string_lossy().into_owned());
                            }
                        }
                    },
                    WordToken::Arithmetic(s) => expand_arithmetic(&mut output, s, &expand_func),
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
                    return array_expand(elements, expand_func, index);
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
                        Select::None => return Array::new(),
                        Select::All => {
                            expand_process(&mut output, command, quoted, Select::All, expand_func);
                            return output.split_whitespace()
                                .map(From::from)
                                .collect::<Array>();
                        },
                        Select::Index(Index::Forward(id)) => {
                            expand_process(&mut output, command, quoted, Select::All, expand_func);
                            return output.split_whitespace()
                                         .nth(id)
                                         .map(Into::into)
                                         .into_iter()
                                         .collect();
                        },
                        Select::Index(Index::Backward(id)) => {
                            expand_process(&mut output, command, quoted, Select::All, expand_func);
                            return output.split_whitespace()
                                         .rev()
                                         .nth(id)
                                         .map(Into::into)
                                         .into_iter()
                                         .collect();
                        }
                        Select::Range(range) => {
                            expand_process(&mut output, command, quoted, Select::All, expand_func);
                            if let Some((start, length)) = range.bounds(output.split_whitespace().count()) {
                                return output.split_whitespace()
                                             .skip(start)
                                             .take(length)
                                             .map(From::from)
                                             .collect();

                            } else {
                                return Array::new();
                            }
                        },
                    }
                },
                WordToken::ArrayMethod(ref array_method) => {

                    return if array_method.returns_array() {
                        array_method.handle_as_array(expand_func)
                    } else {
                        let mut output = String::new();
                        array_method.handle(&mut output, expand_func);
                        Array::from_vec(vec![output])
                    };
                },
                _ => ()
            }
        }

        for word in token_buffer {
            match *word {
                WordToken::Array(ref elements, index) => {
                    output.push_str(&array_expand(elements, expand_func, index).join(" "));
                },
                WordToken::ArrayVariable(array, _, index) => {
                    if let Some(array) = (expand_func.array)(array, index) {
                        output.push_str(&array.join(" "));
                    }
                },
                WordToken::ArrayProcess(command, quoted, index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    match index {
                        Select::None => (),
                        Select::All => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Select::All, expand_func);
                            let temp = temp.split_whitespace().collect::<Vec<&str>>();
                            output.push_str(&temp.join(" "));
                        },
                        Select::Index(Index::Forward(id)) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Select::All, expand_func);
                            output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                        },
                        Select::Index(Index::Backward(id)) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Select::All, expand_func);
                            output.push_str(temp.split_whitespace()
                                                .rev()
                                                .nth(id)
                                                .unwrap_or_default());
                        },
                        Select::Range(range) => {
                            let mut temp = String::new();
                            expand_process(&mut temp, command, quoted, Select::All, expand_func);
                            if let Some((start, length)) = range.bounds(temp.split_whitespace().count()) {
                                let temp = temp.split_whitespace()
                                               .skip(start)
                                               .take(length)
                                               .collect::<Vec<_>>();
                                output.push_str(&temp.join(" "))
                            }
                        },
                    }
                },
                WordToken::ArrayMethod(ref array_method) => {
                    array_method.handle(&mut output, expand_func);
                },
                WordToken::StringMethod(method, variable, pattern, index) => {
                    let pattern = &expand_string(pattern, expand_func, false).join(" ");
                    match method {
                        "join" => if let Some(array) = (expand_func.array)(variable, Select::All) {
                            slice_string(&mut output, &array.join(pattern), index);
                        },
                        "len" => output.push_str(&UnicodeSegmentation::graphemes (
                            expand_func.vars.get_var_or_empty(variable).as_str(), true
                        ).count().to_string()),
                        "len_bytes" => output.push_str(
                            &expand_func.vars.get_var_or_empty(variable).len().to_string()
                        ),
                        _ => {
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "ion: invalid string method: {}", method);
                        }
                    }
                },
                WordToken::Brace(_) => unreachable!(),
                WordToken::Normal(text,false) | WordToken::Whitespace(text) => {
                    output.push_str(text);
                },
                WordToken::Normal(text,true) => {
                    let globbed = glob(text);
                    if let Ok(var)=globbed{
                        is_glob=true;
                        for path in var.filter_map(Result::ok) {
                            expanded_words.push(path.to_string_lossy().into_owned());

                        }
                    }
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
                WordToken::Arithmetic(s) => expand_arithmetic(&mut output, s, expand_func),
            }
        }
        //the is_glob variable can probably be removed, I'm not entirely sure if empty strings are valid in any case- maarten
        if !(is_glob && output == "") {
            expanded_words.push(output.into());
        }
    }

    expanded_words
}

/// Expand a string inside an arithmetic expression, for example:
/// ```ignore
/// x * 5 + y => 22
/// ```
/// if `x=5` and `y=7`
fn expand_arithmetic(output : &mut String , input : &str, expander : &ExpanderFunctions) {
    let mut intermediate = String::with_capacity(input.as_bytes().len());
    let mut varbuf = String::new();
    let flush = |var : &mut String, out : &mut String| {
        if ! var.is_empty() {
            // We have reached the end of a potential variable, so we expand it and push
            // it onto the result
            let res = (expander.variable)(&var, false);
            match res {
                Some(v) => out.push_str(&v),
                None => out.push_str(&var),
            }
            var.clear();
        }
    };
    for c in input.bytes() {
        match c {
            48...57 | 65...90 | 95 | 97...122 => {
                varbuf.push(c as char);
            },
            _ => {
                flush(&mut varbuf, &mut intermediate);
                intermediate.push(c as char);
            }
        }
    }
    flush(&mut varbuf, &mut intermediate);
    match calc::eval(&intermediate) {
        Ok(s) => output.push_str(&(s.to_string())),
        Err(e) => {
            let err_string : String = e.into();
            output.push_str(&err_string);
        }
    }
}

// TODO: Write Nested Brace Tests

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! functions {
        () => {
            ExpanderFunctions {
                vars:     &Variables::default(),
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
        {
            let expected = Array::new();
            let idxs = vec!["-17", "4..-4"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander, false));
            }
        }
    }

    #[test]
    fn embedded_array_expansion() {
        let line = |idx : &str| format!("[[foo bar] [baz bat] [bing crosby]][{}]", idx);
        let expander = functions!();
        let cases : Vec<(Vec<String>, &str)> = vec![
            (vec!["foo".into()], "0"),
            (vec!["baz".into()], "2"),
            (vec!["bat".into()], "-3"),
            (vec!["bar".into(), "baz".into(), "bat".into()], "1...3")
        ];
        for (expected, idx) in cases {
            assert_eq!(Array::from_vec(expected), expand_string(&line(idx), &expander, false));
        }
    }

    #[test]
    fn arith_expression() {
        let line = "$((A * A - (A + A)))";
        let expected = Array::from_vec(vec!["-1".to_owned()]);
        assert_eq!(expected, expand_string(line, &functions!(), false));
        let line = "$((3 * 10 - 27))";
        let expected = Array::from_vec(vec!["3".to_owned()]);
        assert_eq!(expected, expand_string(line, &functions!(), false));
    }
}
