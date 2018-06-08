// TODO: Handle Runtime Errors
extern crate calc;
extern crate permutate;

mod braces;
mod ranges;
mod words;

pub(crate) use self::words::{Index, Range, Select, WordIterator, WordToken};
use self::{braces::BraceToken, ranges::parse_range};
use glob::glob;
use std::{ptr, str};
use types::*;
use unicode_segmentation::UnicodeSegmentation;

/// Determines whether an input string is expression-like as compared to a
/// bare word. For example, strings starting with '"', '\'', '@', or '$' are
/// all expressions
pub(crate) fn is_expression(s: &str) -> bool {
    s.starts_with('@')
        || s.starts_with('[')
        || s.starts_with('$')
        || s.starts_with('"')
        || s.starts_with('\'')
}

/// Trait representing different elements of string expansion
pub(crate) trait Expander {
    /// Expand a tilde form to the correct directory
    fn tilde(&self, &str) -> Option<String> { None }
    /// Expand an array variable with some selection
    fn array(&self, &str, Select) -> Option<Array> { None }
    /// Expand a string variable given if its quoted / unquoted
    fn variable(&self, &str, bool) -> Option<Value> { None }
    /// Expand a subshell expression
    fn command(&self, &str) -> Option<Value> { None }
}

fn expand_process<E: Expander>(
    current: &mut String,
    command: &str,
    selection: Select,
    expander: &E,
    quoted: bool,
) {
    if let Some(mut output) = expander.command(command) {
        if output.is_empty() {
            return;
        } else if quoted {
            let output: &str = if let Some(pos) = output.rfind(|x| x != '\n') {
                &output[..pos + 1]
            } else {
                &output
            };
            slice(current, output, selection)
        } else {
            // If we ever do something with UTF-8, this won't work
            unsafe {
                let bytes = output.as_bytes_mut();
                let bytes_v = bytes.as_mut_ptr();
                let mut size = bytes.len();
                let mut i = 0;
                let mut prev_is_whitespace = true;
                while i < size {
                    let is_whitespace = char::is_whitespace(bytes[i] as char);
                    if is_whitespace {
                        bytes[i] = b' ';
                    }
                    if is_whitespace && prev_is_whitespace {
                        size -= 1;
                        if i != size - 1 {
                            let offset = i as isize;
                            ptr::copy(bytes_v.offset(offset + 1), bytes_v.offset(offset), size - i);
                        }
                    } else {
                        i += 1;
                        prev_is_whitespace = is_whitespace;
                    }
                }
                if prev_is_whitespace {
                    size -= 1;
                }
                slice(current, str::from_utf8_unchecked(&bytes[..size]), selection)
            }
        }
    }
}

fn expand_brace<E: Expander>(
    current: &mut String,
    expanders: &mut Vec<Vec<String>>,
    tokens: &mut Vec<BraceToken>,
    nodes: &[&str],
    expand_func: &E,
    reverse_quoting: bool,
) {
    let mut temp = Vec::new();
    for word in nodes
        .into_iter()
        .flat_map(|node| expand_string(node, expand_func, reverse_quoting))
    {
        match parse_range(&word) {
            Some(elements) => for word in elements {
                temp.push(word.into())
            },
            None => temp.push(word.into()),
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

fn array_expand<E: Expander>(elements: &[&str], expand_func: &E, selection: Select) -> Array {
    match selection {
        Select::None => Array::new(),
        Select::All => elements
            .iter()
            .flat_map(|e| expand_string(e, expand_func, false))
            .collect(),
        Select::Index(index) => array_nth(elements, expand_func, index)
            .into_iter()
            .collect(),
        Select::Range(range) => array_range(elements, expand_func, range),
        Select::Key(_) => Array::new(),
    }
}

fn array_nth<E: Expander>(elements: &[&str], expand_func: &E, index: Index) -> Option<Value> {
    let mut expanded = elements
        .iter()
        .flat_map(|e| expand_string(e, expand_func, false));
    match index {
        Index::Forward(n) => expanded.nth(n),
        Index::Backward(n) => expanded.rev().nth(n),
    }
}

fn array_range<E: Expander>(elements: &[&str], expand_func: &E, range: Range) -> Array {
    let expanded = elements
        .iter()
        .flat_map(|e| expand_string(e, expand_func, false))
        .collect::<Array>();
    let len = expanded.len();
    if let Some((start, length)) = range.bounds(len) {
        expanded.into_iter().skip(start).take(length).collect()
    } else {
        Array::new()
    }
}

fn slice<S: AsRef<str>>(output: &mut String, expanded: S, selection: Select) {
    match selection {
        Select::None => (),
        Select::All => output.push_str(expanded.as_ref()),
        Select::Index(Index::Forward(id)) => if let Some(character) =
            UnicodeSegmentation::graphemes(expanded.as_ref(), true).nth(id)
        {
            output.push_str(character);
        },
        Select::Index(Index::Backward(id)) => if let Some(character) =
            UnicodeSegmentation::graphemes(expanded.as_ref(), true)
                .rev()
                .nth(id)
        {
            output.push_str(character);
        },
        Select::Range(range) => {
            let graphemes = UnicodeSegmentation::graphemes(expanded.as_ref(), true);
            if let Some((start, length)) = range.bounds(graphemes.clone().count()) {
                let substring = graphemes
                    .skip(start)
                    .take(length)
                    .collect::<Vec<&str>>()
                    .join("");
                output.push_str(&substring);
            }
        }
        Select::Key(_) => (),
    }
}

/// Performs shell expansions to an input string, efficiently returning the final
/// expanded form. Shells must provide their own batteries for expanding tilde
/// and variable words.
pub(crate) fn expand_string<E: Expander>(
    original: &str,
    expand_func: &E,
    reverse_quoting: bool,
) -> Array {
    let mut token_buffer = Vec::new();
    let mut contains_brace = false;
    let mut word_iterator = WordIterator::new(original, expand_func);

    loop {
        match word_iterator.next() {
            Some(word) => {
                if let WordToken::Brace(_) = word {
                    contains_brace = true;
                }
                token_buffer.push(word);
            }
            None if original.is_empty() => {
                token_buffer.push(WordToken::Normal("", true, false));
                break;
            }
            None => break,
        }
    }

    expand_tokens(&token_buffer, expand_func, reverse_quoting, contains_brace)
}

fn expand_braces<E: Expander>(
    word_tokens: &[WordToken],
    expand_func: &E,
    reverse_quoting: bool,
) -> Array {
    let mut expanded_words = Array::new();
    let mut output = String::new();
    let tokens: &mut Vec<BraceToken> = &mut Vec::new();
    let mut expanders: Vec<Vec<String>> = Vec::new();

    for word in word_tokens {
        match *word {
            WordToken::Array(ref elements, ref index) => {
                output.push_str(&array_expand(elements, expand_func, index.clone()).join(" "));
            }
            WordToken::ArrayVariable(array, _, ref index) => {
                if let Some(array) = expand_func.array(array, index.clone()) {
                    output.push_str(&array.join(" "));
                }
            }
            WordToken::ArrayProcess(command, _, ref index) => match *index {
                Select::None => (),
                Select::All => {
                    let mut temp = String::new();
                    expand_process(&mut temp, command, Select::All, expand_func, false);
                    let temp = temp.split_whitespace().collect::<Vec<&str>>();
                    output.push_str(&temp.join(" "));
                }
                Select::Index(Index::Forward(id)) => {
                    let mut temp = String::new();
                    expand_process(&mut temp, command, Select::All, expand_func, false);
                    output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                }
                Select::Index(Index::Backward(id)) => {
                    let mut temp = String::new();
                    expand_process(&mut temp, command, Select::All, expand_func, false);
                    output.push_str(temp.split_whitespace().rev().nth(id).unwrap_or_default());
                }
                Select::Range(range) => {
                    let mut temp = String::new();
                    expand_process(&mut temp, command, Select::All, expand_func, false);
                    let len = temp.split_whitespace().count();
                    if let Some((start, length)) = range.bounds(len) {
                        let res = temp
                            .split_whitespace()
                            .skip(start)
                            .take(length)
                            .collect::<Vec<&str>>();
                        output.push_str(&res.join(" "));
                    }
                }
                Select::Key(_) => (),
            },
            WordToken::ArrayMethod(ref method) => {
                method.handle(&mut output, expand_func);
            }
            WordToken::StringMethod(ref method) => {
                method.handle(&mut output, expand_func);
            }
            WordToken::Brace(ref nodes) => expand_brace(
                &mut output,
                &mut expanders,
                tokens,
                nodes,
                expand_func,
                reverse_quoting,
            ),
            WordToken::Whitespace(whitespace) => output.push_str(whitespace),
            WordToken::Process(command, quoted, ref index) => {
                let quoted = if reverse_quoting { !quoted } else { quoted };
                expand_process(&mut output, command, index.clone(), expand_func, quoted);
            }
            WordToken::Variable(text, quoted, ref index) => {
                let quoted = if reverse_quoting { !quoted } else { quoted };
                let expanded = match expand_func.variable(text, quoted) {
                    Some(var) => var,
                    None => continue,
                };

                slice(&mut output, expanded, index.clone());
            }
            WordToken::Normal(text, _, tilde) => {
                expand(
                    &mut output,
                    &mut expanded_words,
                    expand_func,
                    text,
                    false,
                    tilde,
                );
            }
            WordToken::Arithmetic(s) => expand_arithmetic(&mut output, s, expand_func),
        }
    }

    if expanders.is_empty() {
        expanded_words.push(output.into());
    } else {
        if !output.is_empty() {
            tokens.push(BraceToken::Normal(output));
        }
        let tmp: Vec<Vec<&str>> = expanders
            .iter()
            .map(|list| list.iter().map(AsRef::as_ref).collect::<Vec<&str>>())
            .collect();
        let vector_of_arrays: Vec<&[&str]> = tmp.iter().map(AsRef::as_ref).collect();
        for word in braces::expand_braces(&tokens, &*vector_of_arrays) {
            expanded_words.push(word.into());
        }
    }

    expanded_words.into_iter().fold(Array::new(), |mut array, word| {
        if let Some(_) = word.find('*') {
            if let Ok(paths) = glob(&word) {
                for path in paths {
                    if let Ok(path_buf) = path {
                        array.push(path_buf.to_string_lossy().to_string());
                    } else {
                        array.push(String::new());
                    }
                }
            } else {
                array.push(word);
            }
            array
        } else {
            array.push(word);
            array
        }
    })
}

fn expand_single_array_token<E: Expander>(token: &WordToken, expand_func: &E) -> Option<Array> {
    let mut output = String::new();
    match *token {
        WordToken::Array(ref elements, ref index) => {
            Some(array_expand(elements, expand_func, index.clone()))
        }
        WordToken::ArrayVariable(array, quoted, ref index) => {
            match expand_func.array(array, index.clone()) {
                Some(ref array) if quoted => Some(array.join(" ").into()).into_iter().collect(),
                Some(array) => Some(array),
                None => Some(Array::new()),
            }
        }
        WordToken::ArrayProcess(command, _, ref index) => match *index {
            Select::None => Some(Array::new()),
            Select::All => {
                expand_process(&mut output, command, Select::All, expand_func, false);
                Some(output.split_whitespace().map(From::from).collect::<Array>())
            }
            Select::Index(Index::Forward(id)) => {
                expand_process(&mut output, command, Select::All, expand_func, false);
                Some(
                    output
                        .split_whitespace()
                        .nth(id)
                        .map(Into::into)
                        .into_iter()
                        .collect(),
                )
            }
            Select::Index(Index::Backward(id)) => {
                expand_process(&mut output, command, Select::All, expand_func, false);
                Some(
                    output
                        .split_whitespace()
                        .rev()
                        .nth(id)
                        .map(Into::into)
                        .into_iter()
                        .collect(),
                )
            }
            Select::Range(range) => {
                expand_process(&mut output, command, Select::All, expand_func, false);
                if let Some((start, length)) = range.bounds(output.split_whitespace().count()) {
                    Some(
                        output
                            .split_whitespace()
                            .skip(start)
                            .take(length)
                            .map(From::from)
                            .collect(),
                    )
                } else {
                    Some(Array::new())
                }
            }
            Select::Key(_) => Some(Array::new()),
        },
        WordToken::ArrayMethod(ref array_method) => Some(array_method.handle_as_array(expand_func)),
        _ => None,
    }
}

fn expand_single_string_token<E: Expander>(
    token: &WordToken,
    expand_func: &E,
    reverse_quoting: bool,
) -> Array {
    let mut output = String::new();
    let mut expanded_words = Array::new();

    match *token {
        WordToken::StringMethod(ref method) => method.handle(&mut output, expand_func),
        WordToken::Normal(text, do_glob, tilde) => {
            expand(
                &mut output,
                &mut expanded_words,
                expand_func,
                text,
                do_glob,
                tilde,
            );
        }
        WordToken::Whitespace(text) => output.push_str(text),
        WordToken::Process(command, quoted, ref index) => {
            let quoted = if reverse_quoting { !quoted } else { quoted };
            expand_process(&mut output, command, index.clone(), expand_func, quoted);
        }
        WordToken::Variable(text, quoted, ref index) => {
            let quoted = if reverse_quoting { !quoted } else { quoted };
            let expanded = match expand_func.variable(text, quoted) {
                Some(var) => var,
                None => {
                    if output != "" {
                        expanded_words.push(output.into());
                    }
                    return expanded_words;
                }
            };

            slice(&mut output, expanded, index.clone());
        }
        WordToken::Arithmetic(s) => expand_arithmetic(&mut output, s, expand_func),
        _ => unreachable!(),
    }

    if output != "" {
        expanded_words.push(output.into());
    }
    expanded_words
}

fn expand<E: Expander>(
    output: &mut String,
    expanded_words: &mut Array,
    expand_func: &E,
    text: &str,
    do_glob: bool,
    tilde: bool,
) {
    let expanded: String = if tilde {
        match expand_func.tilde(text) {
            Some(s) => s,
            None => text.into(),
        }
    } else {
        text.into()
    };

    if do_glob {
        match glob(&expanded) {
            Ok(var) => {
                let mut globs_found = false;
                for path in var.filter_map(Result::ok) {
                    globs_found = true;
                    expanded_words.push(path.to_string_lossy().into_owned());
                }
                if !globs_found {
                    expanded_words.push(expanded);
                }
            }
            Err(_) => expanded_words.push(expanded),
        }
    } else {
        output.push_str(&expanded);
    }
}

pub(crate) fn expand_tokens<E: Expander>(
    token_buffer: &[WordToken],
    expand_func: &E,
    reverse_quoting: bool,
    contains_brace: bool,
) -> Array {
    if !token_buffer.is_empty() {
        if contains_brace {
            return expand_braces(&token_buffer, expand_func, reverse_quoting);
        } else if token_buffer.len() == 1 {
            let token = &token_buffer[0];
            return match expand_single_array_token(token, expand_func) {
                Some(array) => array,
                None => expand_single_string_token(token, expand_func, reverse_quoting),
            };
        }

        let mut output = String::new();
        let mut expanded_words = Array::new();

        for word in token_buffer {
            match *word {
                WordToken::Array(ref elements, ref index) => {
                    output.push_str(&array_expand(elements, expand_func, index.clone()).join(" "));
                }
                WordToken::ArrayVariable(array, _, ref index) => {
                    if let Some(array) = expand_func.array(array, index.clone()) {
                        output.push_str(&array.join(" "));
                    }
                }
                WordToken::ArrayProcess(command, _, ref index) => match index.clone() {
                    Select::None => (),
                    Select::All => {
                        let mut temp = String::new();
                        expand_process(&mut temp, command, Select::All, expand_func, false);
                        let temp = temp.split_whitespace().collect::<Vec<&str>>();
                        output.push_str(&temp.join(" "));
                    }
                    Select::Index(Index::Forward(id)) => {
                        let mut temp = String::new();
                        expand_process(&mut temp, command, Select::All, expand_func, false);
                        output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                    }
                    Select::Index(Index::Backward(id)) => {
                        let mut temp = String::new();
                        expand_process(&mut temp, command, Select::All, expand_func, false);
                        output.push_str(temp.split_whitespace().rev().nth(id).unwrap_or_default());
                    }
                    Select::Range(range) => {
                        let mut temp = String::new();
                        expand_process(&mut temp, command, Select::All, expand_func, false);
                        if let Some((start, length)) = range.bounds(temp.split_whitespace().count())
                        {
                            let temp = temp
                                .split_whitespace()
                                .skip(start)
                                .take(length)
                                .collect::<Vec<_>>();
                            output.push_str(&temp.join(" "))
                        }
                    }
                    Select::Key(_) => (),
                },
                WordToken::ArrayMethod(ref method) => {
                    method.handle(&mut output, expand_func);
                }
                WordToken::StringMethod(ref method) => {
                    method.handle(&mut output, expand_func);
                }
                WordToken::Brace(_) => unreachable!(),
                WordToken::Normal(text, do_glob, tilde) => {
                    expand(
                        &mut output,
                        &mut expanded_words,
                        expand_func,
                        text,
                        do_glob,
                        tilde,
                    );
                }
                WordToken::Whitespace(text) => {
                    output.push_str(text);
                }
                WordToken::Process(command, quoted, ref index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    expand_process(&mut output, command, index.clone(), expand_func, quoted);
                }
                WordToken::Variable(text, quoted, ref index) => {
                    let quoted = if reverse_quoting { !quoted } else { quoted };
                    let expanded = match expand_func.variable(text, quoted) {
                        Some(var) => var,
                        None => continue,
                    };

                    slice(&mut output, expanded, index.clone());
                }
                WordToken::Arithmetic(s) => expand_arithmetic(&mut output, s, expand_func),
            }
        }
        // I'm not entirely sure if empty strings are valid in any case- maarten
        if output != "" {
            expanded_words.push(output.into());
        }
        expanded_words
    } else {
        array![]
    }
}

/// Expand a string inside an arithmetic expression, for example:
/// ```ignore
/// x * 5 + y => 22
/// ```
/// if `x=5` and `y=7`
fn expand_arithmetic<E: Expander>(output: &mut String, input: &str, expander: &E) {
    let mut intermediate = String::with_capacity(input.as_bytes().len());
    let mut varbuf = String::new();
    let flush = |var: &mut String, out: &mut String| {
        if !var.is_empty() {
            // We have reached the end of a potential variable, so we expand it and push
            // it onto the result
            let res = expander.variable(&var, false);
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
            }
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
            let err_string: String = e.into();
            output.push_str(&err_string);
        }
    }
}

// TODO: Write Nested Brace Tests

#[cfg(test)]
mod test {
    use super::*;

    struct VariableExpander;

    impl Expander for VariableExpander {
        fn variable(&self, variable: &str, _: bool) -> Option<Value> {
            match variable {
                "A" => Some("1".to_owned()),
                "B" => Some("test".to_owned()),
                "C" => Some("ing".to_owned()),
                "D" => Some("1 2 3".to_owned()),
                "FOO" => Some("FOO".to_owned()),
                "BAR" => Some("BAR".to_owned()),
                _ => None,
            }
        }
    }

    struct CommandExpander;

    impl Expander for CommandExpander {
        fn command(&self, cmd: &str) -> Option<Value> { Some(cmd.to_owned()) }
    }

    #[test]
    fn expand_process_quoted() {
        let mut output = String::new();
        let line = " Mary   had\ta little  \n\t lamb\t";
        expand_process(&mut output, line, Select::All, &CommandExpander, true);
        assert_eq!(output, line);
    }

    #[test]
    fn expand_process_unquoted() {
        let mut output = String::new();
        let line = " Mary   had\ta little  \n\t lamb\t";
        expand_process(&mut output, line, Select::All, &CommandExpander, false);
        assert_eq!(output, "Mary had a little lamb");
    }

    #[test]
    fn expand_variable_normal_variable() {
        let input = "$FOO:NOT:$BAR";
        let expected = "FOO:NOT:BAR";
        let expanded = expand_string(input, &VariableExpander, false);
        assert_eq!(array![expected], expanded);
    }

    #[test]
    fn expand_braces() {
        let line = "pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "prodigal programmer processed prototype procedures proficiently proving \
                        prospective projections";
        let expanded = expand_string(line, &VariableExpander, false);
        assert_eq!(
            expected
                .split_whitespace()
                .map(|x| x.to_owned())
                .collect::<Array>(),
            expanded
        );
    }

    #[test]
    fn expand_braces_v2() {
        let line = "It{{em,alic}iz,erat}e{d,}";
        let expected = "Itemized Itemize Italicized Italicize Iterated Iterate";
        let expanded = expand_string(line, &VariableExpander, false);
        assert_eq!(
            expected
                .split_whitespace()
                .map(|x| x.to_owned())
                .collect::<Array>(),
            expanded
        );
    }

    #[test]
    fn expand_variables_with_colons() {
        let expanded = expand_string("$FOO:$BAR", &VariableExpander, false);
        assert_eq!(array!["FOO:BAR"], expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let expanded = expand_string("${B}${C}...${D}", &VariableExpander, false);
        assert_eq!(array!["testing...1 2 3"], expanded);
    }

    #[test]
    fn expand_variable_alongside_braces() {
        let line = "$A{1,2}";
        let expected = array!["11", "12"];
        let expanded = expand_string(line, &VariableExpander, false);
        assert_eq!(expected, expanded);
    }

    #[test]
    fn expand_variable_within_braces() {
        let line = "1{$A,2}";
        let expected = array!["11", "12"];
        let expanded = expand_string(line, &VariableExpander, false);
        assert_eq!(&expected, &expanded);
    }

    #[test]
    fn array_indexing() {
        let base = |idx: &str| format!("[1 2 3][{}]", idx);
        let expander = VariableExpander;
        {
            let expected = array!["1"];
            let idxs = vec!["-3", "0", "..-2"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander, false));
            }
        }
        {
            let expected = array!["2", "3"];
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
        let line = |idx: &str| format!("[[foo bar] [baz bat] [bing crosby]][{}]", idx);
        let expander = VariableExpander;
        let cases = vec![
            (array!["foo"], "0"),
            (array!["baz"], "2"),
            (array!["bat"], "-3"),
            (array!["bar", "baz", "bat"], "1...3"),
        ];
        for (expected, idx) in cases {
            assert_eq!(expected, expand_string(&line(idx), &expander, false));
        }
    }

    #[test]
    fn arith_expression() {
        let line = "$((A * A - (A + A)))";
        let expected = array!["-1"];
        assert_eq!(expected, expand_string(line, &VariableExpander, false));
        let line = "$((3 * 10 - 27))";
        let expected = array!["3"];
        assert_eq!(expected, expand_string(line, &VariableExpander, false));
    }

    #[test]
    fn inline_expression() {
        let cases = vec![
            (array!["5"], "$len([0 1 2 3 4])"),
            (array!["FxOxO"], "$join(@chars(FOO), 'x')"),
        ];
        for (expected, input) in cases {
            assert_eq!(expected, expand_string(input, &VariableExpander, false));
        }
    }
}
