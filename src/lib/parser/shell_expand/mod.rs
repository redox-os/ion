// TODO: Handle Runtime Errors
mod words;

pub(crate) use self::words::{Select, WordIterator, WordToken};
use crate::{
    braces::{self, BraceToken},
    ranges::{parse_range, Index, Range},
    types::{self, Args},
};
use auto_enums::auto_enum;
use glob::glob;
use itertools::Itertools;
use small;
use std::{iter, str};
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

fn join_with_spaces<S: AsRef<str>>(input: &mut small::String, mut iter: impl Iterator<Item = S>) {
    if let Some(str) = iter.next() {
        input.push_str(str.as_ref());
        iter.for_each(|str| {
            input.push(' ');
            input.push_str(str.as_ref());
        });
    }
}

// TODO: Make array expansions iterators instead of arrays.
// TODO: Use Cow<'a, types::Str> for hashmap values.
/// Trait representing different elements of string expansion
pub(crate) trait Expander: Sized {
    /// Expand a tilde form to the correct directory.
    fn tilde(&self, _input: &str) -> Option<String> { None }
    /// Expand an array variable with some selection.
    fn array(&self, _name: &str, _selection: &Select) -> Option<Args> { None }
    /// Expand a string variable given if it's quoted / unquoted
    fn string(&self, _name: &str) -> Option<types::Str> { None }
    /// Expand a subshell expression.
    fn command(&self, _command: &str) -> Option<types::Str> { None }
    /// Iterating upon key-value maps.
    fn map_keys<'a>(&'a self, _name: &str, _select: &Select) -> Option<Args> { None }
    /// Iterating upon key-value maps.
    fn map_values<'a>(&'a self, _name: &str, _select: &Select) -> Option<Args> { None }
    /// Get a string that exists in the shell.
    fn get_string(&self, value: &str) -> types::Str { expand_string(value, self).join(" ").into() }
    /// Select the proper values from an iterator
    fn select<I: Iterator<Item = types::Str>>(vals: I, select: &Select, n: usize) -> Option<Args> {
        match select {
            Select::All => Some(vals.collect()),
            Select::Range(range) => range
                .bounds(n)
                .filter(|&(start, _)| n > start)
                .map(|(start, length)| vals.skip(start).take(length).collect()),
            _ => None,
        }
    }
    /// Get an array that exists in the shell.
    fn get_array(&self, value: &str) -> Args { expand_string(value, self) }
}

fn expand_process<E: Expander>(
    current: &mut small::String,
    command: &str,
    selection: &Select,
    expander: &E,
) {
    if let Some(ref output) = expander.command(command).filter(|out| !out.is_empty()) {
        // Get the pos of the last newline character, then slice them off.
        slice(current, output.trim_end_matches('\n'), &selection);
    }
}

fn expand_brace<E: Expander>(
    current: &mut small::String,
    expanders: &mut Vec<Vec<small::String>>,
    tokens: &mut Vec<BraceToken>,
    nodes: &[&str],
    expand_func: &E,
) {
    let mut temp = Vec::new();
    for word in nodes.iter().flat_map(|node| expand_string_no_glob(node, expand_func)) {
        match parse_range(&word) {
            Some(elements) => temp.extend(elements),
            None => temp.push(word),
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

fn array_expand<E: Expander>(
    elements: &[&str],
    expand_func: &E,
    selection: &Select,
) -> types::Args {
    match selection {
        Select::All => elements.iter().flat_map(|e| expand_string(e, expand_func)).collect(),
        Select::Index(index) => array_nth(elements, expand_func, *index).into_iter().collect(),
        Select::Range(range) => array_range(elements, expand_func, *range),
        Select::Key(_) | Select::None => types::Args::new(),
    }
}

fn array_nth<E: Expander>(elements: &[&str], expand_func: &E, index: Index) -> Option<types::Str> {
    let mut expanded = elements.iter().flat_map(|e| expand_string(e, expand_func));
    match index {
        Index::Forward(n) => expanded.nth(n),
        Index::Backward(n) => expanded.rev().nth(n),
    }
}

fn array_range<E: Expander>(elements: &[&str], expand_func: &E, range: Range) -> types::Args {
    let expanded =
        elements.iter().flat_map(|e| expand_string(e, expand_func)).collect::<types::Args>();
    if let Some((start, length)) = range.bounds(expanded.len()) {
        expanded.into_iter().skip(start).take(length).collect()
    } else {
        types::Args::new()
    }
}

fn slice<S: AsRef<str>>(output: &mut small::String, expanded: S, selection: &Select) {
    match selection {
        Select::All => output.push_str(expanded.as_ref()),
        Select::Index(Index::Forward(id)) => {
            if let Some(character) =
                UnicodeSegmentation::graphemes(expanded.as_ref(), true).nth(*id)
            {
                output.push_str(character);
            }
        }
        Select::Index(Index::Backward(id)) => {
            if let Some(character) =
                UnicodeSegmentation::graphemes(expanded.as_ref(), true).rev().nth(*id)
            {
                output.push_str(character);
            }
        }
        Select::Range(range) => {
            let graphemes = UnicodeSegmentation::graphemes(expanded.as_ref(), true);
            if let Some((start, length)) = range.bounds(graphemes.clone().count()) {
                graphemes.skip(start).take(length).for_each(|str| {
                    output.push_str(str.as_ref());
                });
            }
        }
        Select::Key(_) | Select::None => (),
    }
}

/// Performs shell expansions to an input string, efficiently returning the final
/// expanded form. Shells must provide their own batteries for expanding tilde
/// and variable words.
pub(crate) fn expand_string<E: Expander>(original: &str, expand_func: &E) -> types::Args {
    let mut token_buffer = Vec::new();
    let mut contains_brace = false;

    for word in WordIterator::new(original, expand_func, true) {
        match word {
            WordToken::Brace(_) => {
                contains_brace = true;
                token_buffer.push(word);
            }
            WordToken::ArrayVariable(data, contains_quote, selection) => {
                if let Select::Key(key) = selection {
                    if key.contains(' ') {
                        let keys = key.split(' ');
                        token_buffer.reserve(2 * keys.size_hint().0);
                        for index in keys {
                            let select = index.parse::<Select>().unwrap_or(Select::None);
                            token_buffer.push(WordToken::ArrayVariable(
                                data,
                                contains_quote,
                                select,
                            ));
                            token_buffer.push(WordToken::Whitespace(" "));
                        }
                        token_buffer.pop(); // Pop out the last unneeded whitespace token
                    } else {
                        token_buffer.push(WordToken::ArrayVariable(
                            data,
                            contains_quote,
                            Select::Key(key),
                        ));
                    }
                } else {
                    token_buffer.push(WordToken::ArrayVariable(data, contains_quote, selection));
                }
            }
            _ => token_buffer.push(word),
        }
    }

    if original.is_empty() {
        token_buffer.push(WordToken::Normal("".into(), true, false));
    }
    expand_tokens(&token_buffer, expand_func, contains_brace)
}

fn expand_string_no_glob<E: Expander>(original: &str, expand_func: &E) -> types::Args {
    let mut token_buffer = Vec::new();
    let mut contains_brace = false;

    for word in WordIterator::new(original, expand_func, false) {
        if let WordToken::Brace(_) = word {
            contains_brace = true;
        }
        token_buffer.push(word);
    }
    if original.is_empty() {
        token_buffer.push(WordToken::Normal("".into(), true, false));
    }
    expand_tokens(&token_buffer, expand_func, contains_brace)
}

fn expand_braces<E: Expander>(word_tokens: &[WordToken], expand_func: &E) -> types::Args {
    let mut expanded_words = types::Args::new();
    let mut output = small::String::new();
    let tokens: &mut Vec<BraceToken> = &mut Vec::new();
    let mut expanders: Vec<Vec<small::String>> = Vec::new();

    {
        let output = &mut output;
        crate::IonPool::string(|temp| {
            for word in word_tokens {
                match *word {
                    WordToken::Array(ref elements, ref index) => {
                        join_with_spaces(
                            output,
                            array_expand(elements, expand_func, &index).iter(),
                        );
                    }
                    WordToken::ArrayVariable(array, _, ref index) => {
                        if let Some(array) = expand_func.array(array, index) {
                            join_with_spaces(output, array.iter());
                        }
                    }
                    WordToken::ArrayProcess(command, _, ref index) => match *index {
                        Select::All => {
                            expand_process(temp, command, &Select::All, expand_func);
                            output.push_str(&temp);
                        }
                        Select::Index(Index::Forward(id)) => {
                            expand_process(temp, command, &Select::All, expand_func);
                            output.push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                        }
                        Select::Index(Index::Backward(id)) => {
                            expand_process(temp, command, &Select::All, expand_func);
                            output.push_str(
                                temp.split_whitespace().rev().nth(id).unwrap_or_default(),
                            );
                        }
                        Select::Range(range) => {
                            expand_process(temp, command, &Select::All, expand_func);
                            let len = temp.split_whitespace().count();
                            if let Some((start, length)) = range.bounds(len) {
                                join_with_spaces(
                                    output,
                                    temp.split_whitespace().skip(start).take(length),
                                );
                            }
                        }
                        Select::Key(_) | Select::None => (),
                    },
                    WordToken::ArrayMethod(ref method, _) => {
                        method.handle(output, expand_func);
                    }
                    WordToken::StringMethod(ref method) => {
                        method.handle(output, expand_func);
                    }
                    WordToken::Brace(ref nodes) => {
                        expand_brace(output, &mut expanders, tokens, nodes, expand_func)
                    }
                    WordToken::Whitespace(whitespace) => output.push_str(whitespace),
                    WordToken::Process(command, ref index) => {
                        expand_process(output, command, &index, expand_func);
                    }
                    WordToken::Variable(text, ref index) => {
                        if let Some(expanded) = expand_func.string(text) {
                            slice(output, expanded, &index);
                        };
                    }
                    WordToken::Normal(ref text, _, tilde) => {
                        expand(
                            output,
                            &mut expanded_words,
                            expand_func,
                            text.as_ref(),
                            false,
                            tilde,
                        );
                    }
                    WordToken::Arithmetic(s) => expand_arithmetic(output, s, expand_func),
                }

                temp.clear();
            }
        });
    }

    if expanders.is_empty() {
        expanded_words.push(output);
    } else {
        if !output.is_empty() {
            tokens.push(BraceToken::Normal(output));
        }
        let tmp: Vec<Vec<&str>> = expanders
            .iter()
            .map(|list| list.iter().map(AsRef::as_ref).collect::<Vec<&str>>())
            .collect();
        let vector_of_arrays: Vec<&[&str]> = tmp.iter().map(AsRef::as_ref).collect();
        expanded_words.extend(braces::expand(&tokens, &*vector_of_arrays));
    }

    expanded_words.into_iter().fold(types::Args::new(), |mut array, word| {
        if word.find('*').is_some() {
            if let Ok(paths) = glob(&word) {
                array.extend(paths.map(|path| {
                    if let Ok(path_buf) = path {
                        (*path_buf.to_string_lossy()).into()
                    } else {
                        "".into()
                    }
                }))
            } else {
                array.push(word);
            }
        } else {
            array.push(word);
        }
        array
    })
}

#[auto_enum]
fn expand_single_array_token<E: Expander>(
    token: &WordToken,
    expand_func: &E,
) -> Option<types::Args> {
    match *token {
        WordToken::Array(ref elements, ref index) => {
            Some(array_expand(elements, expand_func, &index))
        }
        WordToken::ArrayVariable(array, quoted, ref index) => {
            match expand_func.array(array, index) {
                Some(ref array) if quoted => Some(args![small::String::from(array.join(" "))]),
                Some(array) => Some(array),
                None => Some(types::Args::new()),
            }
        }
        WordToken::ArrayProcess(command, quoted, ref index) => {
            crate::IonPool::string(|output| match *index {
                Select::Key(_) | Select::None => Some(types::Args::new()),
                _ => {
                    expand_process(output, command, &Select::All, expand_func);

                    #[auto_enum(Iterator)]
                    let mut iterator = match *index {
                        Select::All => output.split_whitespace().map(From::from),
                        Select::Index(Index::Forward(id)) => {
                            output.split_whitespace().nth(id).map(Into::into).into_iter()
                        }
                        Select::Index(Index::Backward(id)) => {
                            output.split_whitespace().rev().nth(id).map(Into::into).into_iter()
                        }
                        Select::Range(range) => {
                            #[auto_enum(Iterator)]
                            match range.bounds(output.split_whitespace().count()) {
                                None => iter::empty(),
                                Some((start, length)) => output
                                    .split_whitespace()
                                    .skip(start)
                                    .take(length)
                                    .map(From::from),
                            }
                        }
                        Select::Key(_) | Select::None => unreachable!(),
                    };

                    if quoted {
                        Some(args!(iterator.join(" ")))
                    } else {
                        Some(iterator.collect())
                    }
                }
            })
        }
        WordToken::ArrayMethod(ref array_method, quoted) => {
            let result = array_method.handle_as_array(expand_func);
            if quoted {
                Some(args!(result.join(" ")))
            } else {
                Some(result)
            }
        }
        _ => None,
    }
}

fn expand_single_string_token<E: Expander>(token: &WordToken, expand_func: &E) -> types::Args {
    let mut output = small::String::new();
    let mut expanded_words = types::Args::new();

    match *token {
        WordToken::StringMethod(ref method) => method.handle(&mut output, expand_func),
        WordToken::Normal(ref text, do_glob, tilde) => {
            expand(&mut output, &mut expanded_words, expand_func, text.as_ref(), do_glob, tilde);
        }
        WordToken::Whitespace(text) => output.push_str(text),
        WordToken::Process(command, ref index) => {
            expand_process(&mut output, command, &index, expand_func);
        }
        WordToken::Variable(text, ref index) => {
            if let Some(expanded) = expand_func.string(text) {
                slice(&mut output, expanded, &index);
            }
        }
        WordToken::Arithmetic(s) => expand_arithmetic(&mut output, s, expand_func),
        _ => unreachable!(),
    }

    if !output.is_empty() {
        expanded_words.push(output);
    }
    expanded_words
}

fn expand<E: Expander>(
    output: &mut small::String,
    expanded_words: &mut types::Args,
    expand_func: &E,
    text: &str,
    do_glob: bool,
    tilde: bool,
) {
    let concat: small::String = match output.rfind(char::is_whitespace) {
        Some(sep) => {
            if sep != output.len() - 1 {
                let word_start = sep + 1;
                let mut t: small::String = output.split_at(word_start).1.into();
                t.push_str(text);
                output.truncate(word_start);
                t
            } else {
                text.into()
            }
        }
        None => {
            if output.is_empty() {
                text.into()
            } else {
                let mut t = output.clone();
                t.push_str(text);
                output.clear();
                t
            }
        }
    };

    let expanded: small::String = if tilde {
        match expand_func.tilde(&concat) {
            Some(s) => s.into(),
            None => concat,
        }
    } else {
        concat
    };

    if do_glob {
        match glob(&expanded) {
            Ok(var) => {
                let prev_size = expanded_words.len();
                expanded_words.extend(
                    var.filter_map(Result::ok).map(|path| path.to_string_lossy().as_ref().into()),
                );
                if expanded_words.len() == prev_size {
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
    contains_brace: bool,
) -> types::Args {
    if !token_buffer.is_empty() {
        if contains_brace {
            return expand_braces(&token_buffer, expand_func);
        } else if token_buffer.len() == 1 {
            let token = &token_buffer[0];
            return match expand_single_array_token(token, expand_func) {
                Some(array) => array,
                None => expand_single_string_token(token, expand_func),
            };
        }

        let mut output = small::String::new();
        let mut expanded_words = types::Args::new();

        {
            let output = &mut output;
            crate::IonPool::string(|temp| {
                for word in token_buffer {
                    match *word {
                        WordToken::Array(ref elements, ref index) => {
                            join_with_spaces(
                                output,
                                array_expand(elements, expand_func, &index).iter(),
                            );
                        }
                        WordToken::ArrayVariable(array, _, ref index) => {
                            if let Some(array) = expand_func.array(array, index) {
                                join_with_spaces(output, array.iter());
                            }
                        }
                        WordToken::ArrayProcess(command, _, ref index) => match index {
                            Select::All => {
                                expand_process(temp, command, &Select::All, expand_func);
                                output.push_str(&temp);
                            }
                            Select::Index(Index::Forward(id)) => {
                                expand_process(temp, command, &Select::All, expand_func);
                                output
                                    .push_str(temp.split_whitespace().nth(*id).unwrap_or_default());
                            }
                            Select::Index(Index::Backward(id)) => {
                                expand_process(temp, command, &Select::All, expand_func);
                                output.push_str(
                                    temp.split_whitespace().rev().nth(*id).unwrap_or_default(),
                                );
                            }
                            Select::Range(range) => {
                                expand_process(temp, command, &Select::All, expand_func);
                                if let Some((start, length)) =
                                    range.bounds(temp.split_whitespace().count())
                                {
                                    join_with_spaces(
                                        output,
                                        temp.split_whitespace().skip(start).take(length),
                                    );
                                }
                            }
                            Select::Key(_) | Select::None => (),
                        },
                        WordToken::ArrayMethod(ref method, _) => {
                            method.handle(output, expand_func);
                        }
                        WordToken::StringMethod(ref method) => {
                            method.handle(output, expand_func);
                        }
                        WordToken::Brace(_) => unreachable!(),
                        WordToken::Normal(ref text, do_glob, tilde) => {
                            expand(
                                output,
                                &mut expanded_words,
                                expand_func,
                                text.as_ref(),
                                do_glob,
                                tilde,
                            );
                        }
                        WordToken::Whitespace(text) => {
                            output.push_str(text);
                        }
                        WordToken::Process(command, ref index) => {
                            expand_process(output, command, &index, expand_func);
                        }
                        WordToken::Variable(text, ref index) => {
                            if let Some(expanded) = expand_func.string(text) {
                                slice(output, expanded, &index);
                            }
                        }
                        WordToken::Arithmetic(s) => expand_arithmetic(output, s, expand_func),
                    }

                    temp.clear();
                }
            });
        }

        if !output.is_empty() {
            expanded_words.insert(0, output);
        }
        expanded_words
    } else {
        Args::new()
    }
}

/// Expand a string inside an arithmetic expression, for example:
/// ```ignore
/// x * 5 + y => 22
/// ```
/// if `x=5` and `y=7`
fn expand_arithmetic<E: Expander>(output: &mut small::String, input: &str, expander: &E) {
    crate::IonPool::string(|intermediate| {
        crate::IonPool::string(|varbuf| {
            let flush = |var: &mut small::String, out: &mut small::String| {
                if !var.is_empty() {
                    // We have reached the end of a potential variable, so we expand it and push
                    // it onto the result
                    out.push_str(expander.string(&var).as_ref().unwrap_or(var));
                }
            };

            for c in input.bytes() {
                match c {
                    48...57 | 65...90 | 95 | 97...122 => {
                        varbuf.push(c as char);
                    }
                    _ => {
                        flush(varbuf, intermediate);
                        varbuf.clear();
                        intermediate.push(c as char);
                    }
                }
            }

            flush(varbuf, intermediate);

            output.push_str(&match calc::eval(&intermediate) {
                Ok(s) => s.to_string(),
                Err(e) => e.to_string(),
            });
        });
    });
}

// TODO: Write Nested Brace Tests

#[cfg(test)]
mod test {
    use super::*;

    struct VariableExpander;

    impl Expander for VariableExpander {
        fn string(&self, variable: &str) -> Option<types::Str> {
            match variable {
                "A" => Some("1".into()),
                "B" => Some("test".into()),
                "C" => Some("ing".into()),
                "D" => Some("1 2 3".into()),
                "FOO" => Some("FOO".into()),
                "BAR" => Some("BAR".into()),
                _ => None,
            }
        }
    }

    struct CommandExpander;

    impl Expander for CommandExpander {
        fn command(&self, cmd: &str) -> Option<types::Str> { Some(cmd.into()) }
    }

    #[test]
    fn expand_process_test() {
        let mut output = small::String::new();

        let line = " Mary   had\ta little  \n\t lamb😉😉\t";
        expand_process(&mut output, line, &Select::All, &CommandExpander);
        assert_eq!(output.as_str(), line);

        output.clear();
        let line = "foo not bar😉😉\n\n";
        expand_process(&mut output, line, &Select::All, &CommandExpander);
        assert_eq!(output.as_str(), "foo not bar😉😉");
    }

    #[test]
    fn expand_variable_normal_variable() {
        let input = "$FOO:NOT:$BAR";
        let expected = "FOO:NOT:BAR";
        let expanded = expand_string(input, &VariableExpander);
        assert_eq!(args![expected], expanded);
    }

    #[test]
    fn expand_braces() {
        let line = "pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "prodigal programmer processed prototype procedures proficiently proving \
                        prospective projections";
        let expanded = expand_string(line, &VariableExpander);
        assert_eq!(
            expected.split_whitespace().map(|x| x.into()).collect::<types::Args>(),
            expanded
        );
    }

    #[test]
    fn expand_braces_v2() {
        let line = "It{{em,alic}iz,erat}e{d,}";
        let expected = "Itemized Itemize Italicized Italicize Iterated Iterate";
        let expanded = expand_string(line, &VariableExpander);
        assert_eq!(
            expected.split_whitespace().map(|x| x.into()).collect::<types::Args>(),
            expanded
        );
    }

    #[test]
    fn expand_variables_with_colons() {
        let expanded = expand_string("$FOO:$BAR", &VariableExpander);
        assert_eq!(args!["FOO:BAR"], expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let expanded = expand_string("${B}${C}...${D}", &VariableExpander);
        assert_eq!(args!["testing...1 2 3"], expanded);
    }

    #[test]
    fn expand_variable_alongside_braces() {
        let line = "$A{1,2}";
        let expected = args!["11", "12"];
        let expanded = expand_string(line, &VariableExpander);
        assert_eq!(expected, expanded);
    }

    #[test]
    fn expand_variable_within_braces() {
        let line = "1{$A,2}";
        let expected = args!["11", "12"];
        let expanded = expand_string(line, &VariableExpander);
        assert_eq!(&expected, &expanded);
    }

    #[test]
    fn array_indexing() {
        let base = |idx: &str| format!("[1 2 3][{}]", idx);
        let expander = VariableExpander;
        {
            let expected = args!["1"];
            let idxs = vec!["-3", "0", "..-2"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander));
            }
        }
        {
            let expected = args!["2", "3"];
            let idxs = vec!["1...2", "1...-1"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander));
            }
        }
        {
            let expected = types::Args::new();
            let idxs = vec!["-17", "4..-4"];
            for idx in idxs {
                assert_eq!(expected, expand_string(&base(idx), &expander));
            }
        }
    }

    #[test]
    fn embedded_array_expansion() {
        let line = |idx: &str| format!("[[foo bar] [baz bat] [bing crosby]][{}]", idx);
        let expander = VariableExpander;
        let cases = vec![
            (args!["foo"], "0"),
            (args!["baz"], "2"),
            (args!["bat"], "-3"),
            (args!["bar", "baz", "bat"], "1...3"),
        ];
        for (expected, idx) in cases {
            assert_eq!(expected, expand_string(&line(idx), &expander));
        }
    }

    #[test]
    fn arith_expression() {
        let line = "$((A * A - (A + A)))";
        let expected = args!["-1"];
        assert_eq!(expected, expand_string(line, &VariableExpander));
        let line = "$((3 * 10 - 27))";
        let expected = args!["3"];
        assert_eq!(expected, expand_string(line, &VariableExpander));
    }

    #[test]
    fn inline_expression() {
        let cases =
            vec![(args!["5"], "$len([0 1 2 3 4])"), (args!["FxOxO"], "$join(@chars('FOO') 'x')")];
        for (expected, input) in cases {
            assert_eq!(expected, expand_string(input, &VariableExpander));
        }
    }
}
