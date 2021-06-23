// TODO: Handle Runtime Errors
mod braces;
mod loops;
mod methods;
/// Expand pipelines
pub mod pipelines;
mod words;

use self::braces::BraceToken;
pub use self::{
    loops::ForValueExpression,
    methods::MethodError,
    words::{unescape, Select, SelectWithSize, WordIterator, WordToken},
};
use crate::{
    parser::lexers::assignments::TypeError,
    ranges::{parse_range, Index, Range},
    types::{self, Args},
};
use auto_enums::auto_enum;
use glob::glob;
use itertools::Itertools;
use std::{
    error,
    fmt::{self, Write},
    str,
};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

/// Expansion errored
#[derive(Debug, Error)]
pub enum Error<T: fmt::Debug + error::Error + fmt::Display + 'static> {
    /// Error during method expansion
    #[error("{0}")]
    MethodError(#[source] MethodError),
    /// Wrong type was given
    #[error("{0}")]
    TypeError(#[source] TypeError),
    /// Indexed out of the array bounds
    #[error("invalid index")] // TODO: Add more info
    OutOfBound,
    /// A string key was taken as index for an array
    #[error("can't use key '{0}' on array")] // TODO: Add more info
    KeyOnArray(String),

    /// Unsupported variable namespace
    #[error("namespace '{0}' is unsupported")]
    UnsupportedNamespace(String),
    /// Failed to parse a value as an hexadecimal value
    #[error("could not parse '{0}' as hexadecimal value: {1}")]
    InvalidHex(String, #[source] std::num::ParseIntError),
    /// Could not parse as a valid color
    #[error("could not parse '{0}' as a color")]
    ColorError(String),
    /// No properties given for color
    #[error("no properties given to color")]
    EmptyColor,
    /// The environment variable is not set
    #[error("environment variable '{0}' is not set")]
    UnknownEnv(String),
    /// Variable is not defined
    #[error("Variable does not exist")]
    VarNotFound,

    /// Failed to fetch the user home directory
    #[error("Could not fetch the user home directory")]
    HomeNotFound,
    /// Tilde expansion tried to access an index out of the directory stack size
    #[error("Can't expand tilde: {0} is out of bound for directory stack")]
    OutOfStack(usize),

    /// Subprocess error
    #[error("Could not expand subprocess: {0}")]
    Subprocess(#[source] Box<T>),

    /// Could not parse the index for array or map-like variable
    #[error("Can't parse '{0}' as a valid index for variable")]
    IndexParsingError(String),

    /// Tried to mix types between scalar and array-like variables
    #[error("can't expand a scalar value '{0}' as an array-like")]
    ScalarAsArray(String),

    /// A wrong index was given for indexing variable
    #[error("index '{0:?}' is not valid for {1} variable '{2}'")]
    InvalidIndex(Select<types::Str>, &'static str, String),

    /// Mixed types between maps and scalar/array value
    #[error("variable '{0}' is not a map-like value")]
    NotAMap(String),
}

impl<T: fmt::Display + fmt::Debug + error::Error> From<TypeError> for Error<T> {
    fn from(cause: TypeError) -> Self { Self::TypeError(cause) }
}

impl<T: fmt::Display + fmt::Debug + error::Error> From<MethodError> for Error<T> {
    fn from(cause: MethodError) -> Self { Self::MethodError(cause) }
}

/// The result of expansion with a given expander
pub type Result<T, E> = std::result::Result<T, Error<E>>;

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

// TODO: Make array expansions iterators instead of arrays.
// TODO: Use Cow<'a, types::Str> for hashmap values.
/// Trait representing different elements of string expansion.
pub trait Expander: Sized {
    /// The error returned when command expansion fails
    type Error: fmt::Display + fmt::Debug + error::Error + 'static;

    /// Expand a tilde form to the correct directory.
    fn tilde(&self, _input: &str) -> Result<types::Str, Self::Error>;
    /// Expand an array variable with some selection.
    fn array(&self, _name: &str, _selection: &Select<types::Str>) -> Result<Args, Self::Error>;
    /// Expand a string variable given if it's quoted / unquoted
    fn string(&self, _name: &str) -> Result<types::Str, Self::Error>;
    /// Expand a subshell expression.
    fn command(
        &mut self,
        _command: &str,
        _set_cmd_duration: bool,
    ) -> Result<types::Str, Self::Error>;
    /// Iterating upon key-value maps.
    fn map_keys(&self, _name: &str) -> Result<Args, Self::Error>;
    /// Iterating upon key-value maps.
    fn map_values(&self, _name: &str) -> Result<Args, Self::Error>;
    /// Get a string that exists in the shell.
    fn get_string(&mut self, value: &str) -> Result<types::Str, Self::Error> {
        Ok(self.expand_string(value)?.join(" ").into())
    }

    /// Get an array that exists in the shell.
    fn get_array(&mut self, value: &str) -> Result<Args, Self::Error> { self.expand_string(value) }

    /// Performs shell expansions to an input string, efficiently returning the final
    /// expanded form. Shells must provide their own batteries for expanding tilde
    /// and variable words.
    fn expand_string(&mut self, original: &str) -> Result<Args, Self::Error> {
        if original.is_empty() {
            return Ok(args![""]);
        }

        let mut token_buffer = Vec::new();
        let mut contains_brace = false;

        for word in WordIterator::new(original, true) {
            if let WordToken::Brace(_) = word {
                contains_brace = true;
            }
            token_buffer.push(word)
        }

        self.expand_tokens(&token_buffer, contains_brace)
    }
}

impl<T: Expander> ExpanderInternal for T {}

trait ExpanderInternal: Expander {
    fn expand_process<'a>(
        &mut self,
        current: &mut types::Str,
        command: &str,
        selection: &Option<&'a str>,
    ) -> Result<(), Self::Error> {
        let result = self.command(command, true)?;
        self.slice(current, result.trim_end_matches('\n'), selection)
    }

    fn expand_brace(
        &mut self,
        current: &mut types::Str,
        expanders: &mut Vec<Vec<types::Str>>,
        tokens: &mut Vec<BraceToken>,
        nodes: &[&str],
    ) -> Result<(), Self::Error> {
        let mut temp = Vec::new();
        for node in nodes {
            let expansions = self.expand_string_no_glob(node)?;
            for word in expansions {
                match parse_range(&word) {
                    Some(elements) => temp.extend(elements),
                    None => temp.push(word),
                }
            }
        }
        if temp.is_empty() {
            current.push_str("{}");
        } else {
            if !current.is_empty() {
                tokens.push(BraceToken::Normal(current.clone()));
                current.clear();
            }
            tokens.push(BraceToken::Expander);
            expanders.push(temp);
        }
        Ok(())
    }

    fn array_expand(
        &mut self,
        elements: &[&str],
        selection: &Option<&str>,
    ) -> Result<Args, Self::Error> {
        let selection = if let Some(selection) = selection {
            let value = self.expand_string(selection)?.join(" ");
            value.parse::<Select<types::Str>>().map_err(|_| Error::IndexParsingError(value))?
        } else {
            Select::All
        };
        match selection {
            Select::All => {
                let mut collected = Args::new();
                for element in elements {
                    collected.extend(self.expand_string(element)?);
                }
                Ok(collected)
            }
            Select::Index(index) => self.array_nth(elements, index).map(|el| args![el]),
            Select::Range(range) => self.array_range(elements, range),
            Select::Key(_) => Err(Error::OutOfBound),
        }
    }

    fn array_nth(&mut self, elements: &[&str], index: Index) -> Result<types::Str, Self::Error> {
        let mut i = match index {
            Index::Forward(n) | Index::Backward(n) => n,
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
        Err(Error::OutOfBound)
    }

    fn array_range(&mut self, elements: &[&str], range: Range) -> Result<Args, Self::Error> {
        let mut expanded = Args::new();
        for element in elements {
            expanded.extend(self.expand_string(element)?);
        }
        if let Some((start, length)) = range.bounds(expanded.len()) {
            Ok(expanded.into_iter().skip(start).take(length).collect())
        } else {
            Err(Error::OutOfBound)
        }
    }

    fn slice_array<'a, S: Into<types::Str>, T: Iterator<Item = S>>(
        &mut self,
        expanded: T,
        selection: &Option<&'a str>,
    ) -> Result<Args, Self::Error> {
        if let Some(selection) = selection {
            let value = self.expand_string(selection)?.join(" ");
            let selection =
                value.parse::<Select<types::Str>>().map_err(|_| Error::IndexParsingError(value))?;
            let expanded: Vec<_> = expanded.collect();
            let len = expanded.len();
            Ok(expanded.into_iter().map(Into::into).select(&selection, len))
        } else {
            Ok(expanded.map(Into::into).collect())
        }
    }

    fn slice<'a, S: AsRef<str>>(
        &mut self,
        output: &mut types::Str,
        expanded: S,
        selection: &Option<&'a str>,
    ) -> Result<(), Self::Error> {
        if let Some(selection) = selection {
            let value = self.expand_string(selection)?.join(" ");
            let selection =
                value.parse::<Select<types::Str>>().map_err(|_| Error::IndexParsingError(value))?;
            match selection {
                Select::All => output.push_str(expanded.as_ref()),
                Select::Index(Index::Forward(id)) => {
                    if let Some(character) =
                        UnicodeSegmentation::graphemes(expanded.as_ref(), true).nth(id)
                    {
                        output.push_str(character);
                    }
                }
                Select::Index(Index::Backward(id)) => {
                    if let Some(character) =
                        UnicodeSegmentation::graphemes(expanded.as_ref(), true).rev().nth(id)
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
                Select::Key(_) => (),
            }
        } else {
            output.push_str(expanded.as_ref())
        };
        Ok(())
    }

    fn expand_string_no_glob(&mut self, original: &str) -> Result<Args, Self::Error> {
        let mut token_buffer = Vec::new();
        let mut contains_brace = false;

        for word in WordIterator::new(original, false) {
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

    #[auto_enum]
    fn expand_single_array_token(&mut self, token: &WordToken<'_>) -> Result<Args, Self::Error> {
        match *token {
            WordToken::Array(ref elements, ref index) => {
                self.array_expand(elements, index).map_err(Into::into)
            }
            WordToken::ArrayVariable(array, quoted, Some(key)) if key.contains(' ') => {
                if quoted {
                    let mut output = types::Str::new();
                    for index in key.split(' ') {
                        let value = self.expand_string(index)?.join(" ");
                        let select = value
                            .parse::<Select<types::Str>>()
                            .map_err(|_| Error::IndexParsingError(value))?;
                        let _ = write!(
                            &mut output,
                            "{}",
                            self.array(array, &select)?.iter().format(" ")
                        );
                        output.push(' ');
                    }
                    output.pop(); // Pop out the last unneeded whitespace token
                    Ok(args![output])
                } else {
                    let mut out = Args::with_capacity(10);
                    for index in key.split(' ') {
                        let value = self.expand_string(index)?.join(" ");
                        let select = value
                            .parse::<Select<types::Str>>()
                            .map_err(|_| Error::IndexParsingError(value))?;
                        out.extend(self.array(array, &select)?);
                    }
                    Ok(out)
                }
            }
            WordToken::ArrayVariable(array, quoted, ref index) => {
                let index = if let Some(index) = index {
                    let value = self.expand_string(index)?.join(" ");
                    value
                        .parse::<Select<types::Str>>()
                        .map_err(|_| Error::IndexParsingError(value))?
                } else {
                    Select::All
                };
                let array = self.array(array, &index)?;
                if quoted {
                    Ok(args![types::Str::from(array.join(" "))])
                } else {
                    Ok(array)
                }
            }
            WordToken::ArrayProcess(command, quoted, ref index) => {
                crate::IonPool::string(|output| {
                    self.expand_process(output, command, &None)?;

                    if quoted {
                        Ok(args!(format!(
                            "{}",
                            self.slice_array(output.split_whitespace(), index)?
                                .into_iter()
                                .format(" ")
                        )))
                    } else {
                        self.slice_array(output.split_whitespace(), index)
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

    fn expand_single_string_token(&mut self, token: &WordToken<'_>) -> Result<Args, Self::Error> {
        let mut output = types::Str::new();
        let mut expanded_words = Args::new();

        match *token {
            WordToken::StringMethod(ref method) => method.handle(&mut output, self)?,
            WordToken::Normal(ref text, do_glob, tilde) => {
                self.expand(&mut output, &mut expanded_words, text.as_ref(), do_glob, tilde)?
            }
            WordToken::Whitespace(text) => output.push_str(text),
            WordToken::Process(command, ref index) => {
                self.expand_process(&mut output, command, index)?
            }
            WordToken::Variable(text, ref index) => {
                self.slice(&mut output, self.string(text)?, index)?;
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
        output: &mut types::Str,
        expanded_words: &mut Args,
        text: &str,
        do_glob: bool,
        tilde: bool,
    ) -> Result<(), Self::Error> {
        let concat: types::Str = match output.rfind(char::is_whitespace) {
            Some(sep) => {
                if sep == output.len() - 1 {
                    text.into()
                } else {
                    let word_start = sep + 1;
                    let mut t: types::Str = output.split_at(word_start).1.into();
                    t.push_str(text);
                    output.truncate(word_start);
                    t
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

        let expanded: types::Str = if tilde { self.tilde(&concat)? } else { concat };

        if do_glob {
            match glob(&expanded) {
                Ok(var) => {
                    let prev_size = expanded_words.len();
                    expanded_words
                        .extend(var.filter_map(|path| path.ok()?.to_str().map(Into::into)));
                    if expanded_words.len() == prev_size {
                        expanded_words.push(expanded);
                    }
                }
                Err(_) => expanded_words.push(expanded),
            }
        } else {
            output.push_str(&expanded);
        }
        Ok(())
    }

    fn expand_tokens(
        &mut self,
        token_buffer: &[WordToken<'_>],
        contains_brace: bool,
    ) -> Result<Args, Self::Error> {
        if !contains_brace && token_buffer.len() == 1 {
            let token = &token_buffer[0];
            return self.expand_single_array_token(token);
        }

        let mut output = types::Str::new();
        let mut expanded_words = Args::new();
        let tokens: &mut Vec<BraceToken> = &mut Vec::new();
        let mut expanders: Vec<Vec<types::Str>> = Vec::new();

        for word in token_buffer {
            match word {
                WordToken::Array(ref elements, ref index) => {
                    let _ = write!(
                        &mut output,
                        "{}",
                        self.array_expand(elements, index)?.iter().format(" ")
                    );
                }
                WordToken::ArrayVariable(array, _, Some(key)) if key.contains(' ') => {
                    for index in key.split(' ') {
                        let select = index
                            .parse::<Select<types::Str>>()
                            .map_err(|_| Error::IndexParsingError(index.into()))?;
                        let _ = write!(
                            &mut output,
                            "{}",
                            self.array(array, &select)?.iter().format(" ")
                        );
                        output.push(' ');
                    }
                    output.pop(); // Pop out the last unneeded whitespace token
                }
                WordToken::ArrayVariable(array, _, ref index) => {
                    let index = if let Some(index) = index {
                        let value = self.expand_string(index)?.join(" ");
                        value
                            .parse::<Select<types::Str>>()
                            .map_err(|_| Error::IndexParsingError(value))?
                    } else {
                        Select::All
                    };
                    let _ =
                        write!(&mut output, "{}", self.array(array, &index)?.iter().format(" "));
                }
                WordToken::ArrayProcess(command, _, ref index)
                | WordToken::Process(command, ref index) => {
                    self.expand_process(&mut output, command, index)?;
                }
                WordToken::ArrayMethod(ref method, _) => {
                    method.handle(&mut output, self)?;
                }
                WordToken::StringMethod(ref method) => {
                    method.handle(&mut output, self)?;
                }
                WordToken::Brace(ref nodes) => {
                    self.expand_brace(&mut output, &mut expanders, tokens, nodes)?;
                }
                WordToken::Normal(ref text, do_glob, tilde) => {
                    self.expand(
                        &mut output,
                        &mut expanded_words,
                        text.as_ref(),
                        *do_glob && !contains_brace,
                        *tilde,
                    )?;
                }
                WordToken::Whitespace(text) => {
                    output.push_str(text);
                }
                WordToken::Variable(text, ref index) => {
                    self.slice(&mut output, self.string(text)?, index)?;
                }
                WordToken::Arithmetic(s) => self.expand_arithmetic(&mut output, s),
            }
        }

        if contains_brace {
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
                expanded_words.extend(braces::expand(tokens, &vector_of_arrays));
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
        } else {
            if !output.is_empty() {
                expanded_words.insert(0, output);
            }
            Ok(expanded_words)
        }
    }

    /// Expand a string inside an arithmetic expression, for example:
    /// ```ignore
    /// x * 5 + y => 22
    /// ```
    /// if `x=5` and `y=7`
    fn expand_arithmetic(&self, output: &mut types::Str, input: &str) {
        crate::IonPool::string(|intermediate| {
            crate::IonPool::string(|varbuf| {
                let flush = |var: &mut types::Str, out: &mut types::Str| {
                    if !var.is_empty() {
                        // We have reached the end of a potential variable, so we expand it and push
                        // it onto the result
                        out.push_str(self.string(var).as_ref().unwrap_or(var));
                    }
                };

                for c in input.bytes() {
                    match c {
                        b'0'..=b'9' | b'A'..=b'Z' | b'_' | b'a'..=b'z' => {
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

                output.push_str(&match calc::eval(intermediate) {
                    Ok(s) => s.to_string(),
                    Err(e) => e.to_string(),
                });
            });
        });
    }
}

// TODO: Write Nested Brace Tests

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::shell::IonError;

    pub struct DummyExpander;

    impl Expander for DummyExpander {
        type Error = IonError;

        fn string(&self, variable: &str) -> Result<types::Str, Self::Error> {
            match variable {
                "A" => Ok("1".into()),
                "B" => Ok("test".into()),
                "C" => Ok("ing".into()),
                "D" => Ok("1 2 3".into()),
                "BAR" => Ok("BAR".into()),
                "FOO" => Ok("FOOBAR".into()),
                "SPACEDFOO" => Ok("FOO BAR".into()),
                "MULTILINE" => Ok("FOO\nBAR".into()),
                "pkmn1" => Ok("Pokémon".into()),
                "pkmn2" => Ok("Poke\u{0301}mon".into()),
                "BAZ" => Ok("  BARBAZ   ".into()),
                "EMPTY" => Ok("".into()),
                _ => Err(Error::VarNotFound),
            }
        }

        fn array(
            &self,
            variable: &str,
            _selection: &Select<types::Str>,
        ) -> Result<types::Args, Self::Error> {
            match variable {
                "ARRAY" => Ok(args!["a", "b", "c"].to_owned()),
                _ => Err(Error::VarNotFound),
            }
        }

        fn command(
            &mut self,
            cmd: &str,
            _set_cmd_duration: bool,
        ) -> Result<types::Str, Self::Error> {
            Ok(cmd.into())
        }

        fn tilde(&self, input: &str) -> Result<types::Str, Self::Error> { Ok(input.into()) }

        fn map_keys<'a>(&'a self, _name: &str) -> Result<Args, Self::Error> {
            Err(Error::VarNotFound)
        }

        fn map_values<'a>(&'a self, _name: &str) -> Result<Args, Self::Error> {
            Err(Error::VarNotFound)
        }
    }

    #[test]
    fn expand_process_test() {
        let mut output = types::Str::new();

        let line = " Mary   had\ta little  \n\t lamb😉😉\t";
        DummyExpander.expand_process(&mut output, line, &None).unwrap();
        assert_eq!(output.as_str(), line);

        output.clear();
        let line = "foo not bar😉😉\n\n";
        DummyExpander.expand_process(&mut output, line, &None).unwrap();
        assert_eq!(output.as_str(), "foo not bar😉😉");
    }

    #[test]
    fn expand_variable_normal_variable() {
        let input = "$FOO:NOT:$BAR";
        let expected = "FOOBAR:NOT:BAR";
        let expanded = DummyExpander.expand_string(input).unwrap();
        assert_eq!(args![expected], expanded);
    }

    #[test]
    fn expand_braces() {
        let line = "pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "prodigal programmer processed prototype procedures proficiently proving \
                        prospective projections";
        let expanded = DummyExpander.expand_string(line).unwrap();
        assert_eq!(expected.split_whitespace().map(types::Str::from).collect::<Args>(), expanded);
    }

    #[test]
    fn expand_braces_v2() {
        let line = "It{{em,alic}iz,erat}e{d,}";
        let expected = "Itemized Itemize Italicized Italicize Iterated Iterate";
        let expanded = DummyExpander.expand_string(line).unwrap();
        assert_eq!(expected.split_whitespace().map(types::Str::from).collect::<Args>(), expanded);
    }

    #[test]
    fn expand_variables_with_colons() {
        let expanded = DummyExpander.expand_string("$FOO:$BAR").unwrap();
        assert_eq!(args!["FOOBAR:BAR"], expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let expanded = DummyExpander.expand_string("${B}${C}...${D}").unwrap();
        assert_eq!(args!["testing...1 2 3"], expanded);
    }

    #[test]
    fn expand_variable_alongside_braces() {
        let line = "$A{1,2}";
        let expected = args!["11", "12"];
        let expanded = DummyExpander.expand_string(line).unwrap();
        assert_eq!(expected, expanded);
    }

    #[test]
    fn expand_variable_within_braces() {
        let line = "1{$A,2}";
        let expected = args!["11", "12"];
        let expanded = DummyExpander.expand_string(line).unwrap();
        assert_eq!(&expected, &expanded);
    }

    #[test]
    fn array_indexing() {
        let base = |idx: &str| format!("[1 2 3][{}]", idx);
        for idx in &["-3", "0", "..-2"] {
            let expanded = DummyExpander.expand_string(&base(idx)).unwrap();
            assert_eq!(args!["1"], expanded, "array[{}] == {} != 1", idx, expanded[0]);
        }
        for idx in &["1...2", "1...-1"] {
            assert_eq!(args!["2", "3"], DummyExpander.expand_string(&base(idx)).unwrap());
        }
        for idx in &["-17", "4..-4"] {
            assert!(DummyExpander.expand_string(&base(idx)).is_err());
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
            assert_eq!(expected, DummyExpander.expand_string(&line(idx)).unwrap());
        }
    }

    #[test]
    fn arith_expression() {
        let line = "$((A * A - (A + A)))";
        let expected = args!["-1"];
        assert_eq!(expected, DummyExpander.expand_string(line).unwrap());
        let line = "$((3 * 10 - 27))";
        let expected = args!["3"];
        assert_eq!(expected, DummyExpander.expand_string(line).unwrap());
    }

    #[test]
    fn inline_expression() {
        let cases =
            vec![(args!["5"], "$len([0 1 2 3 4])"), (args!["FxOxO"], "$join(@chars('FOO') 'x')")];
        for (expected, input) in cases {
            assert_eq!(expected, DummyExpander.expand_string(input).unwrap());
        }
    }
}
