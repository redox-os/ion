// TODO: Handle Runtime Errors
mod words;

pub use self::words::{unescape, MethodError, Select, WordIterator, WordToken};
use crate::{
    braces::{self, BraceToken},
    lexers::assignments::TypeError,
    ranges::{parse_range, Index, Range},
    types::{self, Args},
};
use auto_enums::auto_enum;
use err_derive::Error;
use glob::glob;
use itertools::Itertools;
use small;
use std::{iter, str};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Error)]
pub enum ExpansionError {
    #[error(display = "{}", _0)]
    MethodError(#[error(cause)] MethodError),
    #[error(display = "{}", _0)]
    TypeError(#[error(cause)] TypeError),
    #[error(display = "invalid index")] // TODO: Add more info
    OutOfBound,
}

impl From<TypeError> for ExpansionError {
    fn from(cause: TypeError) -> Self { ExpansionError::TypeError(cause) }
}

impl From<MethodError> for ExpansionError {
    fn from(cause: MethodError) -> Self { ExpansionError::MethodError(cause) }
}

pub type Result<T> = std::result::Result<T, ExpansionError>;

/// Determines whether an input string is expression-like as compared to a
/// bare word. For example, strings starting with '"', '\'', '@', or '$' are
/// all expressions
pub fn is_expression(s: &str) -> bool {
    s.starts_with('@')
        || s.starts_with('[')
        || s.starts_with('$')
        || s.starts_with('"')
        || s.starts_with('\'')
}

fn join_with_spaces<S: AsRef<str>, I: IntoIterator<Item = S>>(input: &mut small::String, iter: I) {
    let mut iter = iter.into_iter();
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
pub trait Expander: Sized {
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
    fn get_string(&self, value: &str) -> Result<types::Str> {
        Ok(self.expand_string(value)?.join(" ").into())
    }
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
    fn get_array(&self, value: &str) -> Result<Args> { self.expand_string(value) }

    /// Performs shell expansions to an input string, efficiently returning the final
    /// expanded form. Shells must provide their own batteries for expanding tilde
    /// and variable words.
    fn expand_string(&self, original: &str) -> Result<Args> {
        let mut token_buffer = Vec::new();
        let mut contains_brace = false;

        for word in WordIterator::new(original, self, true) {
            let word = word?;
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
                        token_buffer.push(WordToken::ArrayVariable(
                            data,
                            contains_quote,
                            selection,
                        ));
                    }
                }
                _ => token_buffer.push(word),
            }
        }

        if original.is_empty() {
            token_buffer.push(WordToken::Normal("".into(), true, false));
        }
        self.expand_tokens(&token_buffer, contains_brace)
    }
}

impl<T: Expander> ExpanderInternal for T {}

trait ExpanderInternal: Expander {
    fn expand_process(&self, current: &mut small::String, command: &str, selection: &Select) {
        if let Some(ref output) = self.command(command).filter(|out| !out.is_empty()) {
            Self::slice(current, output.trim_end_matches('\n'), &selection);
        }
    }

    fn expand_brace(
        &self,
        current: &mut small::String,
        expanders: &mut Vec<Vec<small::String>>,
        tokens: &mut Vec<BraceToken>,
        nodes: &[&str],
    ) -> Result<()> {
        let mut temp = Vec::new();
        for node in nodes {
            for word in self.expand_string_no_glob(node)? {
                match parse_range(&word) {
                    Some(elements) => temp.extend(elements),
                    None => temp.push(word),
                }
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
        Ok(())
    }

    fn array_expand(&self, elements: &[&str], selection: &Select) -> Result<Args> {
        match selection {
            Select::All => {
                let mut collected = Args::new();
                for element in elements {
                    collected.extend(self.expand_string(element)?);
                }
                Ok(collected)
            }
            Select::Index(index) => Ok(self.array_nth(elements, *index).into_iter().collect()),
            Select::Range(range) => self.array_range(elements, *range),
            Select::Key(_) | Select::None => Ok(Args::new()),
        }
    }

    fn array_nth(&self, elements: &[&str], index: Index) -> Result<types::Str> {
        let mut i = match index {
            Index::Forward(n) => n,
            Index::Backward(n) => n,
        };
        if let Index::Forward(_) = index {
            for el in elements {
                let mut expanded = self.expand_string(el)?;
                if expanded.len() > i {
                    return Ok(expanded.swap_remove(i));
                }
                i -= expanded.len();
            }
        } else {
            i += 1; // no need to repeat the substraction at each iteration
            for el in elements.iter().rev() {
                let mut expanded = self.expand_string(el)?;
                if expanded.len() >= i {
                    return Ok(expanded.swap_remove(expanded.len() - i));
                }
                i -= expanded.len();
            }
        }
        Err(ExpansionError::OutOfBound)
    }

    fn array_range(&self, elements: &[&str], range: Range) -> Result<Args> {
        let mut expanded = Args::new();
        for element in elements {
            expanded.extend(self.expand_string(element)?);
        }
        if let Some((start, length)) = range.bounds(expanded.len()) {
            Ok(expanded.into_iter().skip(start).take(length).collect())
        } else {
            Ok(Args::new())
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

    fn expand_string_no_glob(&self, original: &str) -> Result<Args> {
        let mut token_buffer = Vec::new();
        let mut contains_brace = false;

        for word in WordIterator::new(original, self, false) {
            let word = word?;
            if let WordToken::Brace(_) = word {
                contains_brace = true;
            }
            token_buffer.push(word);
        }
        if original.is_empty() {
            token_buffer.push(WordToken::Normal("".into(), true, false));
        }
        self.expand_tokens(&token_buffer, contains_brace)
    }

    fn expand_braces(&self, word_tokens: &[WordToken<'_>]) -> Result<Args> {
        let mut expanded_words = Args::new();
        let mut output = small::String::new();
        let tokens: &mut Vec<BraceToken> = &mut Vec::new();
        let mut expanders: Vec<Vec<small::String>> = Vec::new();

        {
            let output = &mut output;
            crate::IonPool::string(|temp| {
                for word in word_tokens {
                    match *word {
                        WordToken::Array(ref elements, ref index) => {
                            match self.array_expand(elements, &index) {
                                Ok(array) => join_with_spaces(output, array),
                                Err(err) => return Err(err),
                            }
                        }
                        WordToken::ArrayVariable(array, _, ref index) => {
                            if let Some(array) = self.array(array, index) {
                                join_with_spaces(output, &array);
                            }
                        }
                        WordToken::ArrayProcess(command, _, ref index) => match *index {
                            Select::All => {
                                self.expand_process(temp, command, &Select::All);
                                output.push_str(&temp);
                            }
                            Select::Index(Index::Forward(id)) => {
                                self.expand_process(temp, command, &Select::All);
                                output
                                    .push_str(temp.split_whitespace().nth(id).unwrap_or_default());
                            }
                            Select::Index(Index::Backward(id)) => {
                                self.expand_process(temp, command, &Select::All);
                                output.push_str(
                                    temp.split_whitespace().rev().nth(id).unwrap_or_default(),
                                );
                            }
                            Select::Range(range) => {
                                self.expand_process(temp, command, &Select::All);
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
                            method.handle(output, self)?;
                        }
                        WordToken::StringMethod(ref method) => {
                            method.handle(output, self)?;
                        }
                        WordToken::Brace(ref nodes) => {
                            self.expand_brace(output, &mut expanders, tokens, nodes)?;
                        }
                        WordToken::Whitespace(whitespace) => output.push_str(whitespace),
                        WordToken::Process(command, ref index) => {
                            self.expand_process(output, command, &index);
                        }
                        WordToken::Variable(text, ref index) => {
                            if let Some(expanded) = self.string(text) {
                                Self::slice(output, expanded, &index);
                            };
                        }
                        WordToken::Normal(ref text, _, tilde) => {
                            self.expand(output, &mut expanded_words, text.as_ref(), false, tilde);
                        }
                        WordToken::Arithmetic(s) => self.expand_arithmetic(output, s),
                    }

                    temp.clear();
                }
                Ok(())
            })?;
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

        Ok(expanded_words.into_iter().fold(Args::new(), |mut array, word| {
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
        }))
    }

    #[auto_enum]
    fn expand_single_array_token(&self, token: &WordToken<'_>) -> Result<Args> {
        match *token {
            WordToken::Array(ref elements, ref index) => self.array_expand(elements, &index),
            WordToken::ArrayVariable(array, quoted, ref index) => match self.array(array, index) {
                Some(ref array) if quoted => Ok(args![small::String::from(array.join(" "))]),
                Some(array) => Ok(array),
                None => Ok(Args::new()),
            },
            WordToken::ArrayProcess(command, quoted, ref index) => {
                crate::IonPool::string(|output| match *index {
                    Select::Key(_) | Select::None => Ok(Args::new()),
                    _ => {
                        self.expand_process(output, command, &Select::All);

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
                            Ok(args!(iterator.join(" ")))
                        } else {
                            Ok(iterator.collect())
                        }
                    }
                })
            }
            WordToken::ArrayMethod(ref array_method, quoted) => {
                let result = array_method.handle_as_array(self)?;
                if quoted {
                    Ok(args!(result.join(" ")))
                } else {
                    Ok(result)
                }
            }
            _ => self.expand_single_string_token(token),
        }
    }

    fn expand_single_string_token(&self, token: &WordToken<'_>) -> Result<Args> {
        let mut output = small::String::new();
        let mut expanded_words = Args::new();

        match *token {
            WordToken::StringMethod(ref method) => method.handle(&mut output, self)?,
            WordToken::Normal(ref text, do_glob, tilde) => {
                self.expand(&mut output, &mut expanded_words, text.as_ref(), do_glob, tilde);
            }
            WordToken::Whitespace(text) => output.push_str(text),
            WordToken::Process(command, ref index) => {
                self.expand_process(&mut output, command, &index);
            }
            WordToken::Variable(text, ref index) => {
                if let Some(expanded) = self.string(text) {
                    Self::slice(&mut output, expanded, &index);
                }
            }
            WordToken::Arithmetic(s) => self.expand_arithmetic(&mut output, s),
            _ => unreachable!(),
        }

        if !output.is_empty() {
            expanded_words.push(output);
        }
        Ok(expanded_words)
    }

    fn expand(
        &self,
        output: &mut small::String,
        expanded_words: &mut Args,
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
            match self.tilde(&concat) {
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
                        var.filter_map(std::result::Result::ok)
                            .map(|path| path.to_string_lossy().as_ref().into()),
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

    fn expand_tokens(&self, token_buffer: &[WordToken<'_>], contains_brace: bool) -> Result<Args> {
        if !token_buffer.is_empty() {
            if contains_brace {
                return self.expand_braces(&token_buffer);
            } else if token_buffer.len() == 1 {
                let token = &token_buffer[0];
                return self.expand_single_array_token(token);
            }

            let mut output = small::String::new();
            let mut expanded_words = Args::new();

            {
                let output = &mut output;
                crate::IonPool::string(|temp| {
                    for word in token_buffer {
                        match *word {
                            WordToken::Array(ref elements, ref index) => {
                                join_with_spaces(
                                    output,
                                    match self.array_expand(elements, &index) {
                                        Ok(el) => el,
                                        Err(err) => return Err(err),
                                    },
                                );
                            }
                            WordToken::ArrayVariable(array, _, ref index) => {
                                if let Some(array) = self.array(array, index) {
                                    join_with_spaces(output, array.iter());
                                }
                            }
                            WordToken::ArrayProcess(command, _, ref index) => match index {
                                Select::All => {
                                    self.expand_process(temp, command, &Select::All);
                                    output.push_str(&temp);
                                }
                                Select::Index(Index::Forward(id)) => {
                                    self.expand_process(temp, command, &Select::All);
                                    output.push_str(
                                        temp.split_whitespace().nth(*id).unwrap_or_default(),
                                    );
                                }
                                Select::Index(Index::Backward(id)) => {
                                    self.expand_process(temp, command, &Select::All);
                                    output.push_str(
                                        temp.split_whitespace().rev().nth(*id).unwrap_or_default(),
                                    );
                                }
                                Select::Range(range) => {
                                    self.expand_process(temp, command, &Select::All);
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
                                method.handle(output, self)?;
                            }
                            WordToken::StringMethod(ref method) => {
                                method.handle(output, self)?;
                            }
                            WordToken::Brace(_) => unreachable!(),
                            WordToken::Normal(ref text, do_glob, tilde) => {
                                self.expand(
                                    output,
                                    &mut expanded_words,
                                    text.as_ref(),
                                    do_glob,
                                    tilde,
                                );
                            }
                            WordToken::Whitespace(text) => {
                                output.push_str(text);
                            }
                            WordToken::Process(command, ref index) => {
                                self.expand_process(output, command, &index);
                            }
                            WordToken::Variable(text, ref index) => {
                                if let Some(expanded) = self.string(text) {
                                    Self::slice(output, expanded, &index);
                                }
                            }
                            WordToken::Arithmetic(s) => self.expand_arithmetic(output, s),
                        }

                        temp.clear();
                    }
                    Ok(())
                })?;
            }

            if !output.is_empty() {
                expanded_words.insert(0, output);
            }
            Ok(expanded_words)
        } else {
            Ok(Args::new())
        }
    }

    /// Expand a string inside an arithmetic expression, for example:
    /// ```ignore
    /// x * 5 + y => 22
    /// ```
    /// if `x=5` and `y=7`
    fn expand_arithmetic(&self, output: &mut small::String, input: &str) {
        crate::IonPool::string(|intermediate| {
            crate::IonPool::string(|varbuf| {
                let flush = |var: &mut small::String, out: &mut small::String| {
                    if !var.is_empty() {
                        // We have reached the end of a potential variable, so we expand it and push
                        // it onto the result
                        out.push_str(self.string(&var).as_ref().unwrap_or(var));
                    }
                };

                for c in input.bytes() {
                    match c {
                        48..=57 | 65..=90 | 95 | 97..=122 => {
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
        CommandExpander.expand_process(&mut output, line, &Select::All);
        assert_eq!(output.as_str(), line);

        output.clear();
        let line = "foo not bar😉😉\n\n";
        CommandExpander.expand_process(&mut output, line, &Select::All);
        assert_eq!(output.as_str(), "foo not bar😉😉");
    }

    #[test]
    fn expand_variable_normal_variable() {
        let input = "$FOO:NOT:$BAR";
        let expected = "FOO:NOT:BAR";
        let expanded = VariableExpander.expand_string(input).unwrap();
        assert_eq!(args![expected], expanded);
    }

    #[test]
    fn expand_braces() {
        let line = "pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "prodigal programmer processed prototype procedures proficiently proving \
                        prospective projections";
        let expanded = VariableExpander.expand_string(line).unwrap();
        assert_eq!(expected.split_whitespace().map(types::Str::from).collect::<Args>(), expanded);
    }

    #[test]
    fn expand_braces_v2() {
        let line = "It{{em,alic}iz,erat}e{d,}";
        let expected = "Itemized Itemize Italicized Italicize Iterated Iterate";
        let expanded = VariableExpander.expand_string(line).unwrap();
        assert_eq!(expected.split_whitespace().map(types::Str::from).collect::<Args>(), expanded);
    }

    #[test]
    fn expand_variables_with_colons() {
        let expanded = VariableExpander.expand_string("$FOO:$BAR").unwrap();
        assert_eq!(args!["FOO:BAR"], expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let expanded = VariableExpander.expand_string("${B}${C}...${D}").unwrap();
        assert_eq!(args!["testing...1 2 3"], expanded);
    }

    #[test]
    fn expand_variable_alongside_braces() {
        let line = "$A{1,2}";
        let expected = args!["11", "12"];
        let expanded = VariableExpander.expand_string(line).unwrap();
        assert_eq!(expected, expanded);
    }

    #[test]
    fn expand_variable_within_braces() {
        let line = "1{$A,2}";
        let expected = args!["11", "12"];
        let expanded = VariableExpander.expand_string(line).unwrap();
        assert_eq!(&expected, &expanded);
    }

    #[test]
    fn array_indexing() {
        let base = |idx: &str| format!("[1 2 3][{}]", idx);
        for idx in vec!["-3", "0", "..-2"] {
            let expanded = VariableExpander.expand_string(&base(idx)).unwrap();
            assert_eq!(args!["1"], expanded, "array[{}] == {} != 1", idx, expanded[0]);
        }
        for idx in vec!["1...2", "1...-1"] {
            assert_eq!(args!["2", "3"], VariableExpander.expand_string(&base(idx)).unwrap());
        }
        for idx in vec!["-17", "4..-4"] {
            assert_eq!(Args::new(), VariableExpander.expand_string(&base(idx)).unwrap());
        }
    }

    #[test]
    fn embedded_array_expansion() {
        let line = |idx: &str| format!("[[foo bar] [baz bat] [bing crosby]][{}]", idx);
        let cases = vec![
            (args!["foo"], "0"),
            (args!["baz"], "2"),
            (args!["bat"], "-3"),
            (args!["bar", "baz", "bat"], "1...3"),
        ];
        for (expected, idx) in cases {
            assert_eq!(expected, VariableExpander.expand_string(&line(idx)).unwrap());
        }
    }

    #[test]
    fn arith_expression() {
        let line = "$((A * A - (A + A)))";
        let expected = args!["-1"];
        assert_eq!(expected, VariableExpander.expand_string(line).unwrap());
        let line = "$((3 * 10 - 27))";
        let expected = args!["3"];
        assert_eq!(expected, VariableExpander.expand_string(line).unwrap());
    }

    #[test]
    fn inline_expression() {
        let cases =
            vec![(args!["5"], "$len([0 1 2 3 4])"), (args!["FxOxO"], "$join(@chars('FOO') 'x')")];
        for (expected, input) in cases {
            assert_eq!(expected, VariableExpander.expand_string(input).unwrap());
        }
    }
}
