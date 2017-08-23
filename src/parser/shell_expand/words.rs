
use std::char;
use std::io::{self, Write};
use std::iter::{FromIterator, empty};
use std::str::FromStr;

use super::{Expander, expand_string};
use super::{is_expression, slice};
use super::ranges::parse_index_range;
use super::super::ArgumentSplitter;
use unicode_segmentation::UnicodeSegmentation;
use shell::plugins::methods::{self, StringMethodPlugins, MethodArguments};

use std::path::Path;
use types::Array;

lazy_static! {
    static ref STRING_METHODS: StringMethodPlugins = methods::collect();
}

// Bit Twiddling Guide:
// var & FLAG != 0 checks if FLAG is enabled
// var & FLAG == 0 checks if FLAG is disabled
// var |= FLAG enables the FLAG
// var &= 255 ^ FLAG disables the FLAG
// var ^= FLAG swaps the state of FLAG

bitflags! {
    pub struct Flags : u8 {
        const BACKSL = 1;
        const SQUOTE = 2;
        const DQUOTE = 4;
        const EXPAND_PROCESSES = 8;
    }
}


/// Index into a vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Index {
    /// Index starting from the beginning of the vector, where `Forward(0)`
    /// is the first element
    Forward(usize),
    /// Index starting from the end of the vector, where `Backward(0)` is the
    /// last element. `
    Backward(usize),
}

impl Index {
    /// Construct an index using the following convetions:
    /// - A positive value `n` represents `Forward(n)`
    /// - A negative value `-n` reprents `Backwards(n - 1)` such that:
    /// ```
    /// assert_eq!(Index::new(-1), Index::Backward(0))
    /// ```
    pub fn new(input: isize) -> Index {
        if input < 0 {
            Index::Backward((input.abs() as usize) - 1)
        } else {
            Index::Forward(input.abs() as usize)
        }
    }

    pub fn resolve(&self, vector_length: usize) -> Option<usize> {
        match *self {
            Index::Forward(n) => Some(n),
            Index::Backward(n) => if n >= vector_length { None } else { Some(vector_length - (n + 1)) },
        }
    }
}

/// A range of values in a vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Range {
    /// Starting index
    start: Index,
    /// Ending index
    end: Index,
    /// Is this range inclusive? If false, this object represents a half-open
    /// range of [start, end), otherwise [start, end]
    inclusive: bool,
}

impl Range {
    pub fn to(end: Index) -> Range {
        Range {
            start: Index::new(0),
            end,
            inclusive: false,
        }
    }

    pub fn from(start: Index) -> Range {
        Range {
            start,
            end: Index::new(-1),
            inclusive: true,
        }
    }

    pub fn inclusive(start: Index, end: Index) -> Range {
        Range {
            start,
            end,
            inclusive: true,
        }
    }

    pub fn exclusive(start: Index, end: Index) -> Range {
        Range {
            start,
            end,
            inclusive: false,
        }
    }

    /// Returns the bounds of this range as a tuple containing:
    /// - The starting point of the range
    /// - The length of the range
    /// ```
    /// let vec = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
    /// let range = Range::exclusive(Index::new(1), Index::new(5));
    /// let (start, size) = range.bounds(vec.len()).unwrap();
    /// let expected = vec![1, 2, 3, 4];
    /// let selection = vec.iter().skip(start).take(size).collect::<Vec<_>>();
    /// assert_eq!(expected, selection);
    /// ```
    pub fn bounds(&self, vector_length: usize) -> Option<(usize, usize)> {
        if let Some(start) = self.start.resolve(vector_length) {
            if let Some(end) = self.end.resolve(vector_length) {
                if end < start {
                    None
                } else if self.inclusive {
                    Some((start, end - start + 1))
                } else {
                    Some((start, end - start))
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Key {
    key: ::types::Key,
}

impl Key {
    pub fn get(&self) -> &::types::Key { return &self.key; }
}

/// Represents a filter on a vector-like object
#[derive(Debug, PartialEq, Clone)]
pub enum Select {
    /// Select no elements
    None,
    /// Select all elements
    All,
    /// Select a single element based on its index
    Index(Index),
    /// Select a range of elements
    Range(Range),
    /// Select an element by mapped key
    Key(Key),
}

pub trait SelectWithSize {
    type Item;
    fn select<O>(&mut self, Select, usize) -> O where O: FromIterator<Self::Item>;
}

impl<I, T> SelectWithSize for I
    where I: Iterator<Item = T>
{
    type Item = T;
    fn select<O>(&mut self, s: Select, size: usize) -> O
        where O: FromIterator<Self::Item>
    {
        match s {
            Select::None => empty().collect(),
            Select::All => self.collect(),
            Select::Index(idx) => {
                idx.resolve(size)
                    .and_then(|idx| self.nth(idx))
                    .into_iter()
                    .collect()
            }
            Select::Range(range) => {
                if let Some((start, length)) = range.bounds(size) {
                    self.skip(start).take(length).collect()
                } else {
                    empty().collect()
                }
            }
            Select::Key(_) => empty().collect(),
        }
    }
}

impl FromStr for Select {
    type Err = ();
    fn from_str(data: &str) -> Result<Select, ()> {
        if ".." == data {
            return Ok(Select::All);
        }

        if let Ok(index) = data.parse::<isize>() {
            return Ok(Select::Index(Index::new(index)));
        }

        if let Some(range) = parse_index_range(data) {
            return Ok(Select::Range(range));
        }

        Ok(Select::Key(Key { key: data.into() }))
    }
}

#[derive(Debug, PartialEq, Clone)]
enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}


#[derive(Debug, PartialEq, Clone)]
pub struct ArrayMethod<'a> {
    method: &'a str,
    variable: &'a str,
    pattern: Pattern<'a>,
    selection: Select,
}

impl<'a> ArrayMethod<'a> {
    pub fn returns_array(&self) -> bool {
        match self.method {
            "split" | "chars" | "bytes" | "graphemes" => true,
            _ => false,
        }
    }

    pub fn handle<E: Expander>(&self, current: &mut String, expand_func: &E) {
        match self.method {
            "split" => {
                let variable = if let Some(variable) = expand_func.variable(self.variable, false) {
                    variable
                } else if is_expression(self.variable) {
                    expand_string(self.variable, expand_func, false).join(" ")
                } else {
                    return;
                };
                match (&self.pattern, self.selection.clone()) {
                    (&Pattern::StringPattern(pattern), Select::All) => {
                        current.push_str(&variable
                            .split(&expand_string(pattern, expand_func, false).join(" "))
                            .collect::<Vec<&str>>()
                            .join(" "))
                    }
                    (&Pattern::Whitespace, Select::All) => {
                        current.push_str(&variable
                            .split(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .collect::<Vec<&str>>()
                            .join(" "))
                    }
                    (_, Select::None) => (),
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Forward(id))) => {
                        current.push_str(
                            variable
                                .split(&expand_string(pattern, expand_func, false).join(" "))
                                .nth(id)
                                .unwrap_or_default(),
                        )
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Forward(id))) => {
                        current.push_str(
                            variable
                                .split(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .nth(id)
                                .unwrap_or_default(),
                        )
                    }
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Backward(id))) => {
                        current.push_str(
                            variable
                                .rsplit(&expand_string(pattern, expand_func, false).join(" "))
                                .nth(id)
                                .unwrap_or_default(),
                        )
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Backward(id))) => {
                        current.push_str(
                            variable
                                .rsplit(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .nth(id)
                                .unwrap_or_default(),
                        )
                    }
                    (&Pattern::StringPattern(pattern), Select::Range(range)) => {
                        let expansion = expand_string(pattern, expand_func, false).join(" ");
                        let iter = variable.split(&expansion);
                        if let Some((start, length)) = range.bounds(iter.clone().count()) {
                            let range = iter.skip(start).take(length).collect::<Vec<_>>().join(" ");
                            current.push_str(&range)
                        }
                    }
                    (&Pattern::Whitespace, Select::Range(range)) => {
                        let len = variable
                            .split(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .count();
                        if let Some((start, length)) = range.bounds(len) {
                            let range = variable
                                .split(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .skip(start)
                                .take(length)
                                .collect::<Vec<&str>>()
                                .join(" ");
                            current.push_str(&range);
                        }

                    }
                    (_, Select::Key(_)) => (),
                }
            }
            _ => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: invalid array method: {}", self.method);
            }
        }
    }

    pub fn handle_as_array<E: Expander>(&self, expand_func: &E) -> Array {

        macro_rules! resolve_var {
            () => {
                if let Some(variable) = expand_func.variable(self.variable, false) {
                    variable
                } else if is_expression(self.variable) {
                    expand_string(self.variable, expand_func, false).join(" ")
                } else {
                    "".into()
                }
            }
        }

        match self.method {
            "split" => {
                let variable = resolve_var!();
                return match (&self.pattern, self.selection.clone()) {
                    (_, Select::None) => Some("".into()).into_iter().collect(),
                    (&Pattern::StringPattern(pattern), Select::All) => {
                        variable
                            .split(&expand_string(pattern, expand_func, false).join(" "))
                            .map(From::from)
                            .collect()
                    }
                    (&Pattern::Whitespace, Select::All) => {
                        variable
                            .split(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .map(From::from)
                            .collect()
                    }
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Forward(id))) => {
                        variable
                            .split(&expand_string(pattern, expand_func, false).join(" "))
                            .nth(id)
                            .map(From::from)
                            .into_iter()
                            .collect()
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Forward(id))) => {
                        variable
                            .split(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .nth(id)
                            .map(From::from)
                            .into_iter()
                            .collect()
                    }
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Backward(id))) => {
                        variable
                            .rsplit(&expand_string(pattern, expand_func, false).join(" "))
                            .nth(id)
                            .map(From::from)
                            .into_iter()
                            .collect()
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Backward(id))) => {
                        variable
                            .rsplit(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .nth(id)
                            .map(From::from)
                            .into_iter()
                            .collect()
                    }
                    (&Pattern::StringPattern(pattern), Select::Range(range)) => {
                        let expansion = expand_string(pattern, expand_func, false).join(" ");
                        let iter = variable.split(&expansion);
                        if let Some((start, length)) = range.bounds(iter.clone().count()) {
                            iter.skip(start).take(length).map(From::from).collect()
                        } else {
                            Array::new()
                        }
                    }
                    (&Pattern::Whitespace, Select::Range(range)) => {
                        let len = variable
                            .split(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .count();
                        if let Some((start, length)) = range.bounds(len) {
                            variable
                                .split(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .skip(start)
                                .take(length)
                                .map(From::from)
                                .collect()
                        } else {
                            Array::new()
                        }
                    }
                    (_, Select::Key(_)) => Some("".into()).into_iter().collect(),
                };
            }
            "graphemes" => {
                let variable = resolve_var!();
                let graphemes = UnicodeSegmentation::graphemes(variable.as_str(), true);
                let len = graphemes.clone().count();
                return graphemes.map(From::from).select(
                    self.selection.clone(),
                    len,
                );
            }
            "bytes" => {
                let variable = resolve_var!();
                let len = variable.as_bytes().len();
                return variable.bytes().map(|b| b.to_string()).select(
                    self.selection
                        .clone(),
                    len,
                );
            }
            "chars" => {
                let variable = resolve_var!();
                let len = variable.chars().count();
                return variable.chars().map(|c| c.to_string()).select(
                    self.selection
                        .clone(),
                    len,
                );
            }
            _ => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: invalid array method: {}", self.method);
            }
        }

        array![]
    }
}

/// Represents a method that operates on and returns a string
#[derive(Debug, PartialEq, Clone)]
pub struct StringMethod<'a> {
    /// Name of this method: currently `join`, `len`, and `len_bytes` are the
    /// supported methods
    method: &'a str,
    /// Variable that this method will operator on. This is a bit of a misnomer
    /// as this can be an expression as well
    variable: &'a str,
    /// Pattern to use for certain methods: currently `join` makes use of a
    /// pattern
    pattern: &'a str,
    /// Selection to use to control the output of this method
    selection: Select,
}

impl<'a> StringMethod<'a> {
    pub fn handle<E: Expander>(&self, output: &mut String, expand: &E) {
        let (variable, pattern) = (self.variable, self.pattern);

        macro_rules! string_eval {
            ($variable:ident $method:tt $pattern:ident) => {{
                let pattern = expand_string($pattern, expand, false).join(" ");
                let is_true = if let Some(value) = expand.variable($variable, false) {
                    value.$method(&pattern)
                } else if is_expression($variable) {
                    expand_string($variable, expand, false).join($pattern)
                        .$method(&pattern)
                } else {
                    false
                };
                output.push_str(if is_true { "1" } else { "0" });
            }}
        }

        macro_rules! path_eval {
            ($method:tt) => {{
                if let Some(value) = expand.variable(variable, false) {
                    output.push_str(Path::new(&value).$method()
                        .and_then(|os_str| os_str.to_str()).unwrap_or(value.as_str()));
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(pattern);
                    output.push_str(Path::new(&word).$method()
                        .and_then(|os_str| os_str.to_str()).unwrap_or(word.as_str()));
                }
            }}
        }

        macro_rules! string_case {
            ($method:tt) => {{
                if let Some(value) = expand.variable(variable, false) {
                    output.push_str(value.$method().as_str());
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(pattern);
                    output.push_str(word.$method().as_str());
                }
            }}
        }

        match self.method {
            "ends_with" => string_eval!(variable ends_with pattern),
            "contains" => string_eval!(variable contains pattern),
            "starts_with" => string_eval!(variable starts_with pattern),
            "basename" => path_eval!(file_name),
            "extension" => path_eval!(extension),
            "filename" => path_eval!(file_stem),
            "parent" => path_eval!(parent),
            "to_lowercase" => string_case!(to_lowercase),
            "to_uppercase" => string_case!(to_uppercase),
            "repeat" => {
                let pattern = expand_string(pattern, expand, false).join(" ");
                match pattern.parse::<usize>() {
                    Ok(repeat) => {
                        if let Some(value) = expand.variable(variable, false) {
                            output.push_str(&value.repeat(repeat));
                        } else if is_expression(variable) {
                            let value = expand_string(variable, expand, false).join(" ");
                            output.push_str(&value.repeat(repeat));
                        }
                    }
                    Err(_) => {
                        eprintln!("ion: value supplied to $repeat() is not a valid number");
                    }
                }
            }
            "replace" => {
                let pattern = ArgumentSplitter::new(pattern)
                    .map(|x| expand_string(x, expand, false).join(" "))
                    .collect::<Vec<_>>();
                if pattern.len() == 2 {
                    if let Some(value) = expand.variable(variable, false) {
                        output.push_str(&value.replace(pattern[0].as_str(), pattern[1].as_str()));
                    } else if is_expression(variable) {
                        let word = expand_string(variable, expand, false).join(" ");
                        output.push_str(&word.replace(pattern[0].as_str(), pattern[1].as_str()));
                    }
                } else {
                    eprintln!("ion: only two patterns can be supplied to $replace()");
                }
            }
            "replacen" => {
                let pattern = ArgumentSplitter::new(pattern)
                    .map(|x| expand_string(x, expand, false).join(" "))
                    .collect::<Vec<_>>();
                if pattern.len() == 3 {
                    if let Ok(nth) = pattern[2].as_str().parse::<usize>() {
                        if let Some(value) = expand.variable(variable, false) {
                            output.push_str(&value.replacen(pattern[0].as_str(), pattern[1].as_str(), nth));
                        } else if is_expression(variable) {
                            let word = expand_string(variable, expand, false).join(" ");
                            output.push_str(&word.replacen(pattern[0].as_str(), pattern[1].as_str(), nth));
                        }
                    } else {
                        eprintln!("ion: the supplied count value is invalid");
                    }
                } else {
                    eprintln!("ion: only three patterns can be supplied to $replacen()");
                }
            }
            "join" => {
                let pattern = expand_string(pattern, expand, false).join(" ");
                if let Some(array) = expand.array(variable, Select::All) {
                    slice(output, array.join(&pattern), self.selection.clone());
                } else if is_expression(variable) {
                    slice(
                        output,
                        expand_string(variable, expand, false).join(&pattern),
                        self.selection.clone(),
                    );
                }
            }
            "len" => {
                if variable.starts_with('@') || variable.starts_with('[') {
                    let expanded = expand_string(variable, expand, false);
                    output.push_str(&expanded.len().to_string());
                } else if let Some(value) = expand.variable(variable, false) {
                    let count = UnicodeSegmentation::graphemes(value.as_str(), true).count();
                    output.push_str(&count.to_string());
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(pattern);
                    let count = UnicodeSegmentation::graphemes(word.as_str(), true).count();
                    output.push_str(&count.to_string());
                }
            }
            "len_bytes" => {
                if let Some(value) = expand.variable(variable, false) {
                    output.push_str(&value.as_bytes().len().to_string());
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(pattern);
                    output.push_str(&word.as_bytes().len().to_string());
                }
            }
            "reverse" => {
                if let Some(value) = expand.variable(variable, false) {
                    let rev_graphs = UnicodeSegmentation::graphemes(value.as_str(), true).rev();
                    output.push_str(rev_graphs.collect::<String>().as_str());
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(pattern);
                    let rev_graphs = UnicodeSegmentation::graphemes(word.as_str(), true).rev();
                    output.push_str(rev_graphs.collect::<String>().as_str());
                }
            }
            method @ _ => {
                let pattern = ArgumentSplitter::new(self.pattern)
                    .flat_map(|arg| expand_string(&arg, expand, false))
                    .collect::<_>();
                let args = if variable.starts_with('@') || variable.starts_with('[') {
                    MethodArguments::Array(
                        expand_string(variable, expand, false).into_vec(),
                        pattern
                    )
                } else if let Some(value) = expand.variable(variable, false) {
                    MethodArguments::StringArg(
                        value,
                        pattern
                    )
                } else if is_expression(variable) {
                    let expanded = expand_string(variable, expand, false);
                    match expanded.len() {
                        0 => MethodArguments::NoArgs,
                        1 => MethodArguments::StringArg(
                            expanded[0].clone(),
                            pattern
                        ),
                        _ => MethodArguments::Array(
                            expanded.into_vec(),
                            pattern
                        )
                    }
                } else {
                    MethodArguments::NoArgs
                };

                match STRING_METHODS.execute(method, args) {
                    Ok(Some(string)) => output.push_str(&string),
                    Ok(None) => (),
                    Err(why) => eprintln!("ion: method plugin: {}", why)
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum WordToken<'a> {
    /// Represents a normal string who may contain a globbing character
    /// (the second element) or a tilde expression (the third element)
    Normal(&'a str, bool, bool),
    Whitespace(&'a str),
    // Tilde(&'a str),
    Brace(Vec<&'a str>),
    Array(Vec<&'a str>, Select),
    Variable(&'a str, bool, Select),
    ArrayVariable(&'a str, bool, Select),
    ArrayProcess(&'a str, bool, Select),
    Process(&'a str, bool, Select),
    StringMethod(StringMethod<'a>),
    ArrayMethod(ArrayMethod<'a>),
    Arithmetic(&'a str), // Glob(&'a str)
}

pub struct WordIterator<'a, E: Expander + 'a> {
    data: &'a str,
    read: usize,
    flags: Flags,
    expanders: &'a E,
}

impl<'a, E: Expander + 'a> WordIterator<'a, E> {
    pub fn new(data: &'a str, expand_processes: bool, expanders: &'a E) -> WordIterator<'a, E> {
        let flags = if expand_processes { EXPAND_PROCESSES } else { Flags::empty() };
        WordIterator {
            data,
            read: 0,
            flags,
            expanders,
        }
    }

    // Contains the grammar for collecting whitespace characters
    fn whitespaces<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        self.read += 1;
        while let Some(character) = iterator.next() {
            if character == b' ' {
                self.read += 1;
            } else {
                return WordToken::Whitespace(&self.data[start..self.read]);
            }
        }

        WordToken::Whitespace(&self.data[start..self.read])
    }

    /// Contains the logic for parsing tilde syntax
    // fn tilde<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    //     where I: Iterator<Item = u8>
    // {
    //     let start = self.read - 1;
    //     while let Some(character) = iterator.next() {
    //         match character {
    //             0...47 | 58...64 | 91...94 | 96 | 123...127 => {
    //                 return WordToken::Tilde(&self.data[start..self.read]);
    //             },
    //             _ => (),
    //         }
    //         self.read += 1;
    //     }
    //
    //     WordToken::Tilde(&self.data[start..])
    // }

    // Contains the logic for parsing braced variables
    fn braced_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        while let Some(character) = iterator.next() {
            if character == b'}' {
                let output = &self.data[start..self.read];
                self.read += 1;
                return WordToken::Variable(output, self.flags.contains(DQUOTE), Select::All);
            }
            self.read += 1;
        }

        // The validator at the frontend should catch unterminated braced variables.
        panic!("ion: fatal error with syntax validation parsing: unterminated braced variable");
    }

    /// Contains the logic for parsing variable syntax
    fn variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let mut start = self.read;
        self.read += 1;
        while let Some(character) = iterator.next() {
            match character {
                b'(' => {
                    let method = &self.data[start..self.read];
                    self.read += 1;
                    start = self.read;
                    let mut depth = 0;
                    while let Some(character) = iterator.next() {
                        match character {
                            b',' if depth == 0 => {
                                let variable = &self.data[start..self.read];
                                self.read += 1;
                                start = self.read;
                                while let Some(character) = iterator.next() {
                                    if character == b')' {
                                        let pattern = &self.data[start..self.read].trim();
                                        self.read += 1;
                                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                            let _ = iterator.next();
                                            WordToken::StringMethod(StringMethod {
                                                method,
                                                variable: variable.trim(),
                                                pattern,
                                                selection: self.read_selection(iterator),
                                            })
                                        } else {
                                            WordToken::StringMethod(StringMethod {
                                                method,
                                                variable: variable.trim(),
                                                pattern,
                                                selection: Select::All,
                                            })
                                        };
                                    }
                                    self.read += 1;
                                }
                            }
                            b')' if depth == 0 => {
                                // If no pattern is supplied, the default is a space.
                                let variable = &self.data[start..self.read];
                                self.read += 1;

                                return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                    let _ = iterator.next();
                                    WordToken::StringMethod(StringMethod {
                                        method,
                                        variable: variable.trim(),
                                        pattern: " ",
                                        selection: self.read_selection(iterator),
                                    })
                                } else {
                                    WordToken::StringMethod(StringMethod {
                                        method,
                                        variable: variable.trim(),
                                        pattern: " ",
                                        selection: Select::All,
                                    })
                                };
                            }
                            b')' => depth -= 1,
                            b'(' => depth += 1,
                            _ => (),
                        }
                        self.read += 1;
                    }

                    panic!("ion: fatal error with syntax validation parsing: unterminated method");
                }
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    let variable = &self.data[start..self.read];

                    return if character == b'[' {
                        WordToken::Variable(variable, self.flags.contains(DQUOTE), self.read_selection(iterator))
                    } else {
                        WordToken::Variable(variable, self.flags.contains(DQUOTE), Select::All)
                    };
                }
                _ => (),
            }
            self.read += 1;
        }

        WordToken::Variable(&self.data[start..], self.flags.contains(DQUOTE), Select::All)
    }

    fn read_selection<I>(&mut self, iterator: &mut I) -> Select
        where I: Iterator<Item = u8>
    {
        self.read += 1;
        let start = self.read;
        while let Some(character) = iterator.next() {
            if let b']' = character {
                let value = expand_string(&self.data[start..self.read], self.expanders, false).join(" ");
                let selection = match value.parse::<Select>() {
                    Ok(selection) => selection,
                    Err(_) => Select::None,
                };
                self.read += 1;
                return selection;
            }
            self.read += 1;
        }

        panic!()
    }

    /// Contains the logic for parsing array variable syntax
    fn array_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let mut start = self.read;
        self.read += 1;
        while let Some(character) = iterator.next() {
            match character {
                b'(' => {
                    let method = &self.data[start..self.read];
                    self.read += 1;
                    start = self.read;
                    let mut depth = 0;
                    while let Some(character) = iterator.next() {
                        match character {
                            b',' if depth == 0 => {
                                let variable = &self.data[start..self.read];
                                self.read += 1;
                                start = self.read;
                                while let Some(character) = iterator.next() {
                                    if character == b')' {
                                        let pattern = &self.data[start..self.read].trim();
                                        self.read += 1;
                                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                            let _ = iterator.next();
                                            WordToken::ArrayMethod(ArrayMethod {
                                                method,
                                                variable: variable.trim(),
                                                pattern: Pattern::StringPattern(pattern),
                                                selection: self.read_selection(iterator),
                                            })
                                        } else {
                                            WordToken::ArrayMethod(ArrayMethod {
                                                method,
                                                variable: variable.trim(),
                                                pattern: Pattern::StringPattern(pattern),
                                                selection: Select::All,
                                            })
                                        };
                                    }
                                    self.read += 1;
                                }
                            }
                            b')' if depth == 0 => {
                                // If no pattern is supplied, the default is a space.
                                let variable = &self.data[start..self.read];
                                self.read += 1;

                                return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                    let _ = iterator.next();
                                    WordToken::ArrayMethod(ArrayMethod {
                                        method,
                                        variable: variable.trim(),
                                        pattern: Pattern::Whitespace,
                                        selection: self.read_selection(iterator),
                                    })
                                } else {
                                    WordToken::ArrayMethod(ArrayMethod {
                                        method,
                                        variable: variable.trim(),
                                        pattern: Pattern::Whitespace,
                                        selection: Select::All,
                                    })
                                };
                            }
                            b')' => depth -= 1,
                            b'(' => depth += 1,
                            _ => (),
                        }
                        self.read += 1;
                    }

                    panic!("ion: fatal error with syntax validation parsing: unterminated method");
                }
                b'[' => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(DQUOTE),
                        self.read_selection(iterator),
                    );
                }
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(DQUOTE),
                        Select::All,
                    );
                }
                _ => (),
            }
            self.read += 1;
        }

        WordToken::ArrayVariable(&self.data[start..], self.flags.contains(DQUOTE), Select::All)
    }

    fn braced_array_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        //self.read += 1;
        while let Some(character) = iterator.next() {
            match character {
                b'[' => {
                    let result = WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(DQUOTE),
                        self.read_selection(iterator),
                    );
                    self.read += 1;
                    if let Some(b'}') = iterator.next() {
                        return result;
                    }
                    panic!("ion: fatal with syntax validation error: unterminated braced array expression");
                }
                b'}' => {
                    let output = &self.data[start..self.read];
                    self.read += 1;
                    return WordToken::ArrayVariable(output, self.flags.contains(DQUOTE), Select::All);
                }
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(DQUOTE),
                        Select::All,
                    );
                }
                _ => (),
            }
            self.read += 1;
        }
        WordToken::ArrayVariable(&self.data[start..], self.flags.contains(DQUOTE), Select::All)
    }

    /// Contains the logic for parsing subshell syntax.
    fn process<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(BACKSL) => self.flags ^= BACKSL,
                b'\\' => self.flags ^= BACKSL,
                b'\'' if !self.flags.contains(DQUOTE) => self.flags ^= SQUOTE,
                b'"' if !self.flags.contains(SQUOTE) => self.flags ^= DQUOTE,
                b'$' if !self.flags.contains(SQUOTE) => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        level += 1;
                    }
                }
                b')' if !self.flags.contains(SQUOTE) => {
                    if level == 0 {
                        let output = &self.data[start..self.read];
                        self.read += 1;
                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::Process(output, self.flags.contains(DQUOTE), self.read_selection(iterator))
                        } else {
                            WordToken::Process(output, self.flags.contains(DQUOTE), Select::All)
                        };
                    } else {
                        level -= 1;
                    }
                }
                _ => (),
            }
            self.read += 1;
        }

        // The validator at the frontend should catch unterminated processes.
        panic!("ion: fatal error with syntax validation: unterminated process");
    }

    /// Contains the logic for parsing array subshell syntax.
    fn array_process<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(BACKSL) => self.flags ^= BACKSL,
                b'\\' => self.flags ^= BACKSL,
                b'\'' if !self.flags.contains(DQUOTE) => self.flags ^= SQUOTE,
                b'"' if !self.flags.contains(SQUOTE) => self.flags ^= DQUOTE,
                b'@' if !self.flags.contains(SQUOTE) => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        level += 1;
                    }
                }
                b')' if !self.flags.contains(SQUOTE) => {
                    if level == 0 {
                        let array_process_contents = &self.data[start..self.read];
                        self.read += 1;
                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::ArrayProcess(
                                array_process_contents,
                                self.flags.contains(DQUOTE),
                                self.read_selection(iterator),
                            )
                        } else {
                            WordToken::ArrayProcess(array_process_contents, self.flags.contains(DQUOTE), Select::All)
                        };
                    } else {
                        level -= 1;
                    }
                }
                _ => (),
            }
            self.read += 1;
        }

        // The validator at the frontend should catch unterminated processes.
        panic!("ion: fatal error with syntax validation: unterminated array process");
    }

    /// Contains the grammar for parsing brace expansion syntax
    fn braces<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let mut start = self.read;
        let mut level = 0;
        let mut elements = Vec::new();
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(BACKSL) => self.flags ^= BACKSL,
                b'\\' => self.flags ^= BACKSL,
                b'\'' if !self.flags.contains(DQUOTE) => self.flags ^= SQUOTE,
                b'"' if !self.flags.contains(SQUOTE) => self.flags ^= DQUOTE,
                b',' if !self.flags.intersects(SQUOTE | DQUOTE) && level == 0 => {
                    elements.push(&self.data[start..self.read]);
                    start = self.read + 1;
                }
                b'{' if !self.flags.intersects(SQUOTE | DQUOTE) => level += 1,
                b'}' if !self.flags.intersects(SQUOTE | DQUOTE) => {
                    if level == 0 {
                        elements.push(&self.data[start..self.read]);
                        self.read += 1;
                        return WordToken::Brace(elements);
                    } else {
                        level -= 1;
                    }
                }
                _ => (),
            }
            self.read += 1;
        }

        panic!("ion: fatal error with syntax validation: unterminated brace")
    }

    /// Contains the grammar for parsing array expression syntax
    fn array<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(BACKSL) => self.flags ^= BACKSL,
                b'\\' => self.flags ^= BACKSL,
                b'\'' if !self.flags.contains(DQUOTE) => self.flags ^= SQUOTE,
                b'"' if !self.flags.contains(SQUOTE) => self.flags ^= DQUOTE,
                b'[' if !self.flags.intersects(SQUOTE | DQUOTE) => level += 1,
                b']' if !self.flags.intersects(SQUOTE | DQUOTE) => {
                    if level == 0 {
                        let elements = ArgumentSplitter::new(&self.data[start..self.read]).collect::<Vec<&str>>();
                        self.read += 1;

                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::Array(elements, self.read_selection(iterator))
                        } else {
                            WordToken::Array(elements, Select::All)
                        };
                    } else {
                        level -= 1;
                    }
                }
                _ => (),
            }
            self.read += 1;
        }

        panic!("ion: fatal error with syntax validation: unterminated array expression")
    }

    fn glob_check<I>(&mut self, iterator: &mut I) -> bool
        where I: Iterator<Item = u8> + Clone
    {
        // Clone the iterator and scan for illegal characters until the corresponding ] is
        // discovered. If none are found, then it's a valid glob signature.
        let mut moves = 0;
        let mut glob = false;
        let mut square_bracket = 0;
        let mut iter = iterator.clone();
        while let Some(character) = iter.next() {
            moves += 1;
            match character {
                b'[' => {
                    square_bracket += 1;
                }
                b' ' | b'"' | b'\'' | b'$' | b'{' | b'}' => {
                    break;
                }
                b']' => {
                    // If the glob is less than three bytes in width, then it's empty and thus invalid.
                    if !(moves <= 3 && square_bracket == 1) {
                        glob = true;
                        break;
                    }
                }
                _ => (),
            }
        }

        if glob {
            for _ in 0..moves {
                iterator.next();
            }
            self.read += moves + 1;
            true
        } else {
            self.read += 1;
            false
        }
    }

    fn arithmetic_expression<I: Iterator<Item = u8>>(&mut self, iter: &mut I) -> WordToken<'a> {
        let mut paren: i8 = 0;
        let start = self.read;
        while let Some(character) = iter.next() {
            match character {
                b'(' => paren += 1,
                b')' => {
                    if paren == 0 {
                        // Skip the incoming ); we have validated this syntax so it should be OK
                        let _ = iter.next();
                        let output = &self.data[start..self.read];
                        self.read += 2;
                        return WordToken::Arithmetic(output);
                    } else {
                        paren -= 1;
                    }
                }
                _ => (),
            }
            self.read += 1;
        }
        panic!("ion: fatal syntax error: unterminated arithmetic expression");
    }
}

impl<'a, E: Expander + 'a> Iterator for WordIterator<'a, E> {
    type Item = WordToken<'a>;

    fn next(&mut self) -> Option<WordToken<'a>> {
        if self.read == self.data.len() {
            return None;
        }

        let mut iterator = self.data.bytes().skip(self.read);
        let mut start = self.read;
        let mut glob = false;
        let mut tilde = false;
        loop {
            if let Some(character) = iterator.next() {
                match character {
                    _ if self.flags.contains(BACKSL) => {
                        self.read += 1;
                        self.flags ^= BACKSL;
                        break;
                    }
                    b'\\' => {
                        if !self.flags.intersects(DQUOTE | SQUOTE) {
                            start += 1;
                        }
                        self.read += 1;
                        self.flags ^= BACKSL;
                        if !self.flags.contains(EXPAND_PROCESSES) {
                            return Some(WordToken::Normal("\\", glob, tilde));
                        }
                        break;
                    }
                    b'\'' if !self.flags.contains(DQUOTE) => {
                        start += 1;
                        self.read += 1;
                        self.flags ^= SQUOTE;
                        if !self.flags.contains(EXPAND_PROCESSES) {
                            return Some(WordToken::Normal("'", glob, tilde));
                        }
                        break;
                    }
                    b'"' if !self.flags.contains(SQUOTE) => {
                        start += 1;
                        self.read += 1;
                        if self.flags.contains(DQUOTE) {
                            self.flags -= DQUOTE;
                            return self.next();
                        }
                        self.flags |= DQUOTE;
                        if !self.flags.contains(EXPAND_PROCESSES) {
                            return Some(WordToken::Normal("\"", glob, tilde));
                        } else {
                            break;
                        }
                    }
                    b' ' if !self.flags.intersects(DQUOTE | SQUOTE) => {
                        return Some(self.whitespaces(&mut iterator));
                    }
                    b'~' if !self.flags.intersects(DQUOTE | SQUOTE) => {
                        tilde = true;
                        self.read += 1;
                        break;
                    }
                    b'{' if !self.flags.intersects(DQUOTE | SQUOTE) => {
                        self.read += 1;
                        return Some(self.braces(&mut iterator));
                    }
                    b'[' if !self.flags.contains(SQUOTE) => {
                        if self.glob_check(&mut iterator) {
                            glob = true;
                        } else {
                            return Some(self.array(&mut iterator));
                        }
                    }
                    b'@' if !self.flags.contains(SQUOTE) => {
                        match iterator.next() {
                            Some(b'(') => {
                                self.read += 2;
                                return if self.flags.contains(EXPAND_PROCESSES) {
                                    Some(self.array_process(&mut iterator))
                                } else {
                                    Some(WordToken::Normal(&self.data[start..self.read], glob, tilde))
                                };
                            }
                            Some(b'{') => {
                                self.read += 2;
                                return Some(self.braced_array_variable(&mut iterator));
                            }
                            _ => {
                                self.read += 1;
                                return Some(self.array_variable(&mut iterator));
                            }
                        }
                    }
                    b'$' if !self.flags.contains(SQUOTE) => {
                        match iterator.next() {
                            Some(b'(') => {
                                self.read += 2;
                                return if self.data.as_bytes()[self.read] == b'(' {
                                    // Pop the incoming left paren
                                    let _ = iterator.next();
                                    self.read += 1;
                                    Some(self.arithmetic_expression(&mut iterator))
                                } else if self.flags.contains(EXPAND_PROCESSES) {
                                    Some(self.process(&mut iterator))
                                } else {
                                    Some(WordToken::Normal(&self.data[start..self.read], glob, tilde))
                                };
                            }
                            Some(b'{') => {
                                self.read += 2;
                                return Some(self.braced_variable(&mut iterator));
                            }
                            _ => {
                                self.read += 1;
                                return Some(self.variable(&mut iterator));
                            }
                        }
                    }
                    b'*' | b'?' => {
                        self.read += 1;
                        glob = true;
                        break;
                    }
                    _ => {
                        self.read += 1;
                        break;
                    }
                }
            } else {
                return None;
            }
        }

        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(BACKSL) => self.flags ^= BACKSL,
                b'\\' => {
                    self.flags ^= BACKSL;
                    let end = if !self.flags.contains(EXPAND_PROCESSES) {
                        if self.flags.intersects(DQUOTE | SQUOTE) {
                            self.read + 2
                        } else {
                            self.read + 1
                        }
                    } else if self.flags.intersects(DQUOTE | SQUOTE) {
                        self.read + 1
                    } else {
                        self.read
                    };
                    let output = &self.data[start..end];
                    self.read += 1;
                    return Some(WordToken::Normal(output, glob, tilde));
                }
                b'\'' if !self.flags.contains(DQUOTE) => {
                    self.flags ^= SQUOTE;
                    let end = if !self.flags.contains(EXPAND_PROCESSES) { self.read + 1 } else { self.read };
                    let output = &self.data[start..end];
                    self.read += 1;
                    return Some(WordToken::Normal(output, glob, tilde));
                }
                b'"' if !self.flags.contains(SQUOTE) => {
                    self.flags ^= DQUOTE;
                    let end = if !self.flags.contains(EXPAND_PROCESSES) { self.read + 1 } else { self.read };
                    let output = &self.data[start..end];
                    self.read += 1;
                    return Some(WordToken::Normal(output, glob, tilde));
                }
                b' ' | b'{' if !self.flags.intersects(SQUOTE | DQUOTE) => {
                    return Some(WordToken::Normal(&self.data[start..self.read], glob, tilde));
                }
                b'$' | b'@' if !self.flags.contains(SQUOTE) => {
                    let output = &self.data[start..self.read];
                    if output != "" {
                        return Some(WordToken::Normal(output, glob, tilde));
                    } else {
                        return self.next();
                    };
                }
                b'[' if !self.flags.contains(SQUOTE) => {
                    if self.glob_check(&mut iterator) {
                        glob = true;
                    } else {
                        return Some(WordToken::Normal(&self.data[start..self.read], glob, tilde));
                    }
                }
                b'*' | b'?' if !self.flags.contains(SQUOTE) => {
                    glob = true;
                }
                b'~' if !self.flags.intersects(SQUOTE | DQUOTE) => {
                    let output = &self.data[start..self.read];
                    if output != "" {
                        return Some(WordToken::Normal(output, glob, tilde));
                    } else {
                        return self.next();
                    }
                }
                _ => (),
            }
            self.read += 1;
        }

        if start == self.read {
            None
        } else {
            Some(WordToken::Normal(&self.data[start..], glob, tilde))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use types::Value;

    struct Empty;

    impl Expander for Empty {}

    fn compare(input: &str, expected: Vec<WordToken>) {
        let mut correct = 0;
        for (actual, expected) in WordIterator::new(input, true, &Empty).zip(expected.iter()) {
            assert_eq!(actual, *expected, "{:?} != {:?}", actual, expected);
            correct += 1;
        }
        assert_eq!(expected.len(), correct);
    }

    #[test]
    fn ranges() {
        let range1 = Range::exclusive(Index::new(1), Index::new(5));
        assert_eq!(Some((1, 4)), range1.bounds(42));
        assert_eq!(Some((1, 4)), range1.bounds(7));
        let range2 = Range::inclusive(Index::new(2), Index::new(-4));
        assert_eq!(Some((2, 5)), range2.bounds(10));
        assert_eq!(None, range2.bounds(3));
    }


    #[test]
    fn string_method() {
        let input = "$join(array, 'pattern') $join(array, 'pattern')";
        let expected = vec![
            WordToken::StringMethod(StringMethod {
                method: "join",
                variable: "array",
                pattern: "'pattern'",
                selection: Select::All,
            }),
            WordToken::Whitespace(" "),
            WordToken::StringMethod(StringMethod {
                method: "join",
                variable: "array",
                pattern: "'pattern'",
                selection: Select::All,
            }),
        ];
        compare(input, expected);
    }

    #[test]
    fn escape_with_backslash() {
        let input = "\\$FOO\\$BAR \\$FOO";
        let expected = vec![
            WordToken::Normal("$FOO", false, false),
            WordToken::Normal("$BAR", false, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("$FOO", false, false),
        ];
        compare(input, expected);
    }

    #[test]
    fn array_expressions() {
        let input = "[ one two [three four]] [[one two] three four][0]";
        let first = vec!["one", "two", "[three four]"];
        let second = vec!["[one two]", "three", "four"];
        let expected = vec![
            WordToken::Array(first, Select::All),
            WordToken::Whitespace(" "),
            WordToken::Array(second, Select::Index(Index::new(0))),
        ];
        compare(input, expected);
    }

    #[test]
    fn array_variables() {
        let input = "@array @array[0] @{array[1..]}";
        let expected = vec![
            WordToken::ArrayVariable("array", false, Select::All),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Select::Index(Index::new(0))),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Select::Range(Range::from(Index::new(1)))),
        ];
        compare(input, expected);
    }

    #[test]
    fn array_processes() {
        let input = "@(echo one two three) @(echo one two three)[0]";
        let expected = vec![
            WordToken::ArrayProcess("echo one two three", false, Select::All),
            WordToken::Whitespace(" "),
            WordToken::ArrayProcess("echo one two three", false, Select::Index(Index::new(0))),
        ];
        compare(input, expected);
    }

    #[test]
    fn indexes() {
        let input = "@array[0..3] @array[0...3] @array[abc] @array[..3] @array[3..]";
        let expected =
            vec![
                WordToken::ArrayVariable("array", false, Select::Range(Range::exclusive(Index::new(0), Index::new(3)))),
                WordToken::Whitespace(" "),
                WordToken::ArrayVariable("array", false, Select::Range(Range::inclusive(Index::new(0), Index::new(3)))),
                WordToken::Whitespace(" "),
                WordToken::ArrayVariable("array", false, Select::Key(Key { key: "abc".into() })),
                WordToken::Whitespace(" "),
                WordToken::ArrayVariable("array", false, Select::Range(Range::to(Index::new(3)))),
                WordToken::Whitespace(" "),
                WordToken::ArrayVariable("array", false, Select::Range(Range::from(Index::new(3)))),
            ];
        compare(input, expected);
    }

    #[test]
    fn string_keys() {
        let input = "@array['key'] @array[key] @array[]";
        let expected = vec![
            WordToken::ArrayVariable("array", false, Select::Key(Key { key: "key".into() })),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Select::Key(Key { key: "key".into() })),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Select::Key(Key { key: "".into() })),
        ];
        compare(input, expected);
    }

    #[test]
    fn nested_processes() {
        let input = "echo $(echo $(echo one)) $(echo one $(echo two) three)";
        let expected = vec![
            WordToken::Normal("echo", false, false),
            WordToken::Whitespace(" "),
            WordToken::Process("echo $(echo one)", false, Select::All),
            WordToken::Whitespace(" "),
            WordToken::Process("echo one $(echo two) three", false, Select::All),
        ];
        compare(input, expected);
    }

    #[test]
    fn words_process_with_quotes() {
        let input = "echo $(git branch | rg '[*]' | awk '{print $2}')";
        let expected = vec![
            WordToken::Normal("echo", false, false),
            WordToken::Whitespace(" "),
            WordToken::Process("git branch | rg '[*]' | awk '{print $2}'", false, Select::All),
        ];
        compare(input, expected);

        let input = "echo $(git branch | rg \"[*]\" | awk '{print $2}')";
        let expected = vec![
            WordToken::Normal("echo", false, false),
            WordToken::Whitespace(" "),
            WordToken::Process("git branch | rg \"[*]\" | awk '{print $2}'", false, Select::All),
        ];
        compare(input, expected);
    }

    #[test]
    fn test_words() {
        let input = "echo $ABC \"${ABC}\" one{$ABC,$ABC} ~ $(echo foo) \"$(seq 1 100)\"";
        let expected = vec![
            WordToken::Normal("echo", false, false),
            WordToken::Whitespace(" "),
            WordToken::Variable("ABC", false, Select::All),
            WordToken::Whitespace(" "),
            WordToken::Variable("ABC", true, Select::All),
            WordToken::Whitespace(" "),
            WordToken::Normal("one", false, false),
            WordToken::Brace(vec!["$ABC", "$ABC"]),
            WordToken::Whitespace(" "),
            WordToken::Normal("~", false, true),
            WordToken::Whitespace(" "),
            WordToken::Process("echo foo", false, Select::All),
            WordToken::Whitespace(" "),
            WordToken::Process("seq 1 100", true, Select::All),
        ];
        compare(input, expected);
    }

    #[test]
    fn test_multiple_escapes() {
        let input = "foo\\(\\) bar\\(\\)";
        let expected = vec![
            WordToken::Normal("foo", false, false),
            WordToken::Normal("(", false, false),
            WordToken::Normal(")", false, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("bar", false, false),
            WordToken::Normal("(", false, false),
            WordToken::Normal(")", false, false),
        ];
        compare(input, expected);
    }

    #[test]
    fn test_arithmetic() {
        let input = "echo $((foo bar baz bing 3 * 2))";
        let expected = vec![
            WordToken::Normal("echo", false, false),
            WordToken::Whitespace(" "),
            WordToken::Arithmetic("foo bar baz bing 3 * 2"),
        ];
        compare(input, expected);
    }

    #[test]
    fn test_globbing() {
        let input = "barbaz* bingcrosb*";
        let expected = vec![
            WordToken::Normal("barbaz*", true, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("bingcrosb*", true, false),
        ];
        compare(input, expected);
    }

    #[test]
    fn test_empty_strings() {
        let input = "rename '' 0 a \"\"";
        let expected = vec![
            WordToken::Normal("rename", false, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("", false, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("0", false, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("a", false, false),
            WordToken::Whitespace(" "),
            WordToken::Normal("", false, false),
        ];
        compare(input, expected);
    }

    struct WithVars;

    impl Expander for WithVars {
        fn variable(&self, var: &str, _: bool) -> Option<Value> {
            match var {
                "pkmn1" => "Pokémon".to_owned().into(),
                "pkmn2" => "Poke\u{0301}mon".to_owned().into(),
                _ => None,
            }
        }
    }

    #[test]
    fn array_methods() {
        let expanders = WithVars;
        let method = ArrayMethod {
            method: "graphemes",
            variable: "pkmn1",
            pattern: Pattern::Whitespace,
            selection: Select::Index(Index::Forward(3)),
        };
        let expected = array!["é"];
        assert_eq!(method.handle_as_array(&expanders), expected);
        let method = ArrayMethod {
            method: "chars",
            variable: "pkmn2",
            pattern: Pattern::Whitespace,
            selection: Select::Index(Index::Forward(3)),
        };
        let expected = array!["e"];
        assert_eq!(method.handle_as_array(&expanders), expected);
        let method = ArrayMethod {
            method: "bytes",
            variable: "pkmn2",
            pattern: Pattern::Whitespace,
            selection: Select::Index(Index::Forward(1)),
        };
        let expected = array!["111"];
        assert_eq!(method.handle_as_array(&expanders), expected);
    }

}
