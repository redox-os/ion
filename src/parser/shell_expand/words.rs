use std::io::{self, Write};
use std::char;
use std::str::FromStr;
use super::{ExpanderFunctions, expand_string};
use super::ranges::parse_index_range;

use types::Array;

// Bit Twiddling Guide:
// var & FLAG != 0 checks if FLAG is enabled
// var & FLAG == 0 checks if FLAG is disabled
// var |= FLAG enables the FLAG
// var &= 255 ^ FLAG disables the FLAG
// var ^= FLAG swaps the state of FLAG

const BACKSL: u8 = 1;
const SQUOTE: u8 = 2;
const DQUOTE: u8 = 4;
const EXPAND_PROCESSES: u8 = 8;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Index {
    All,
    None,
    ID(usize),
    FromEnd(usize),
    Range(IndexStart, IndexEnd),
}

/// Index into an vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum IndexStart {
    /// Index starting from the beginning of the vector, where `FromStart(0)` is the first element
    FromStart(usize),
    /// Index starting from the end of the vector, where `FromEnd(1)` is the last element.
    /// `FromEnd(0)` is a reserved value
    FromEnd(usize)
}

impl IndexStart {

    /// Construct a new input where negative values become `FromEnd` instances and positive
    /// values or zero become FromStart instances
    pub fn new(input : isize) -> IndexStart {
        if input < 0 {
            IndexStart::FromEnd(input.abs() as usize)
        } else {
            IndexStart::FromStart(input.abs() as usize)
        }
    }

    /// `index.resolve(n)` determines the "true" index given the length of a vector `n`
    pub fn resolve(&self, size : usize) -> usize {
        match *self {
            IndexStart::FromStart(n) => n,
            IndexStart::FromEnd(n) => size - n
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum IndexEnd {
    ID(usize),
    FromEnd(usize),
    CatchAll
}

impl IndexEnd {

    /// Construct a new input where negative values become `FromEnd` instances and positive
    /// values or zero become FromStart instances
    pub fn new(input : isize) -> IndexEnd {
        if input < 0 {
            IndexEnd::FromEnd(input.abs() as usize)
        } else {
            IndexEnd::ID(input.abs() as usize)
        }
    }

    /// `index.resolve(n)` determines the "true" index given the length of a vector `n`
    fn resolve(&self, size : usize) -> usize {
        match *self {
            IndexEnd::ID(n) => n,
            IndexEnd::FromEnd(n) => if n > size { 0 } else {size - n},
            IndexEnd::CatchAll => size
        }
    }

    /// Determine the number of values inbetween this index and an IndexStart instance as a range.
    /// If this would result in a reversed range, return zero
    pub fn diff(&self, start : &IndexStart, size : usize) -> usize {
        if self.resolve(size) < start.resolve(size) {
            0
        } else {
            self.resolve(size) - start.resolve(size)
        }
    }
}

pub enum IndexError {
    Invalid
}

impl FromStr for Index {
    type Err = IndexError;
    fn from_str(data: &str) -> Result<Index, IndexError> {
        if ".." == data {
            return Ok(Index::All)
        }

        if let Ok(index) = data.parse::<isize>() {
            if index < 0 {
                return Ok(Index::FromEnd(index.abs() as usize));
            } else {
                return Ok(Index::ID(index.abs() as usize));
            }
        }

        if let Some((start, end)) = parse_index_range(data) {
            return Ok(Index::Range(start, end))
        }

        let stderr = io::stderr();
        let _ = writeln!(stderr.lock(), "ion: supplied index, '{}', for array is invalid", data);

        Err(IndexError::Invalid)
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
    index: Index
}

impl<'a> ArrayMethod<'a> {
    pub fn handle(&self, current: &mut String, expand_func: &ExpanderFunctions) {
        match self.method {
            "split" => if let Some(variable) = (expand_func.variable)(self.variable, false) {
                match (&self.pattern, self.index) {
                    (&Pattern::StringPattern(pattern), Index::All) => current.push_str (
                        &variable.split(&expand_string(pattern, expand_func, false).join(" "))
                            .collect::<Vec<&str>>()
                            .join(" ")
                    ),
                    (&Pattern::Whitespace, Index::All) => current.push_str (
                        &variable.split(char::is_whitespace)
                            .collect::<Vec<&str>>()
                            .join(" ")
                    ),
                    (_, Index::None) => (),
                    (&Pattern::StringPattern(pattern), Index::ID(id)) => current.push_str (
                        variable.split(&expand_string(pattern, expand_func, false).join(" "))
                            .nth(id)
                            .unwrap_or_default()
                    ),
                    (&Pattern::Whitespace, Index::ID(id)) => current.push_str (
                        variable.split(char::is_whitespace)
                            .nth(id)
                            .unwrap_or_default()
                    ),
                    (&Pattern::StringPattern(pattern), Index::FromEnd(id)) => current.push_str (
                        variable.rsplit(&expand_string(pattern, expand_func, false).join(" "))
                            .nth(id - 1)
                            .unwrap_or_default()
                    ),
                    (&Pattern::Whitespace, Index::FromEnd(id)) => current.push_str (
                        variable.rsplit(char::is_whitespace)
                            .nth(id - 1)
                            .unwrap_or_default()
                    ),
                    (&Pattern::StringPattern(pattern), Index::Range(start, end)) => {
                        let expansion = expand_string(pattern, expand_func, false).join(" ");
                        let iter = variable.split(&expansion);
                        let len = iter.clone().count();
                        let range = iter.skip(start.resolve(len))
                                        .take(end.resolve(len) - start.resolve(len))
                                        .collect::<Vec<&str>>()
                                        .join(" ");

                        current.push_str(&range);
                    },
                    (&Pattern::Whitespace, Index::Range(start, end)) => {
                        let len = variable.split(char::is_whitespace).count();
                        let range = variable.split(char::is_whitespace)
                            .skip(start.resolve(len))
                            .take(end.resolve(len) - start.resolve(len))
                            .collect::<Vec<&str>>()
                            .join(" ");

                        current.push_str(&range);
                    },
                }
            },
            _ => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: invalid array method: {}", self.method);
            }
        }
    }

    pub fn handle_as_array(&self, expand_func: &ExpanderFunctions) -> Array {
        match self.method {
            "split" => if let Some(variable) = (expand_func.variable)(self.variable, false) {
                return match (&self.pattern, self.index) {
                    (_, Index::None) => Some("".into()).into_iter().collect(),
                    (&Pattern::StringPattern(pattern), Index::All) => variable
                        .split(&expand_string(pattern, expand_func, false).join(" "))
                        .map(From::from)
                        .collect(),
                    (&Pattern::Whitespace, Index::All) => variable
                        .split(char::is_whitespace)
                        .map(From::from)
                        .collect(),
                    (&Pattern::StringPattern(pattern), Index::ID(id)) =>
                        Some(
                            variable
                                .split(&expand_string(pattern, expand_func, false).join(" "))
                                .nth(id)
                                .map(From::from)
                                .unwrap_or_default()
                        ).into_iter().collect(),
                    (&Pattern::Whitespace, Index::ID(id)) =>
                        Some(
                            variable
                                .split(char::is_whitespace)
                                .nth(id).map(From::from)
                                .unwrap_or_default()
                        ).into_iter().collect(),
                    (&Pattern::StringPattern(pattern), Index::FromEnd(id)) =>
                        Some(
                            variable
                                .rsplit(&expand_string(pattern, expand_func, false).join(" "))
                                .nth(id - 1)
                                .map(From::from)
                                .unwrap_or_default()
                        ).into_iter().collect(),
                    (&Pattern::Whitespace, Index::FromEnd(id)) =>
                        Some(
                            variable
                                .rsplit(char::is_whitespace)
                                .nth(id - 1).map(From::from)
                                .unwrap_or_default()
                        ).into_iter().collect(),
                    (&Pattern::StringPattern(pattern), Index::Range(start, end)) => {
                        let expansion = expand_string(pattern, expand_func, false).join(" ");
                        let iter = variable.split(&expansion);
                        let len = iter.clone().count();
                        iter.skip(start.resolve(len))
                            .take(end.resolve(len) - start.resolve(len))
                            .map(From::from)
                            .collect()
                    },
                    (&Pattern::Whitespace, Index::Range(start, end)) => {
                        let len = variable.split(char::is_whitespace).count();
                        variable.split(char::is_whitespace)
                            .skip(start.resolve(len))
                            .take(end.resolve(len) - start.resolve(len))
                            .map(From::from)
                            .collect()
                    },
                }
            },
            _ => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: invalid array method: {}", self.method);
            }
        }


        Some("".into()).into_iter().collect()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum WordToken<'a> {
    Normal(&'a str),
    Whitespace(&'a str),
    Tilde(&'a str),
    Brace(Vec<&'a str>),
    Array(Vec<&'a str>, Index),
    Variable(&'a str, bool, Index),
    ArrayVariable(&'a str, bool, Index),
    ArrayProcess(&'a str, bool, Index),
    Process(&'a str, bool, Index),
    StringMethod(&'a str, &'a str, &'a str, Index),
    ArrayMethod(ArrayMethod<'a>),
}

pub struct WordIterator<'a> {
    data:          &'a str,
    read:          usize,
    flags:         u8,
}

impl<'a> WordIterator<'a> {
    pub fn new(data: &'a str, expand_processes: bool) -> WordIterator<'a> {
        let flags = if expand_processes { EXPAND_PROCESSES } else { 0 };
        WordIterator { data: data, read: 0, flags: flags }
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
    fn tilde<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read - 1;
        while let Some(character) = iterator.next() {
            match character {
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    return WordToken::Tilde(&self.data[start..self.read]);
                },
                _ => (),
            }
            self.read += 1;
        }

        WordToken::Tilde(&self.data[start..])
    }

    // Contains the logic for parsing braced variables
    fn braced_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        while let Some(character) = iterator.next() {
            if character == b'}' {
                let output = &self.data[start..self.read];
                self.read += 1;
                return WordToken::Variable(output, self.flags & DQUOTE != 0, Index::All);
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
                    while let Some(character) = iterator.next() {
                        if character == b',' {
                            let variable = &self.data[start..self.read];
                            self.read += 1;
                            start = self.read;
                            while let Some(character) = iterator.next() {
                                if character == b')' {
                                    let pattern = &self.data[start..self.read].trim();
                                    self.read += 1;

                                    return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                        let _ = iterator.next();
                                        WordToken::StringMethod(method, variable, pattern, self.read_index(iterator))
                                    } else {
                                        WordToken::StringMethod(method, variable, pattern, Index::All)
                                    };
                                }
                                self.read += 1;
                            }
                        } else if character == b')' {
                            // If no pattern is supplied, the default is a space.
                            let variable = &self.data[start..self.read];
                            self.read += 1;

                            return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                let _ = iterator.next();
                                WordToken::StringMethod(method, variable, " ", self.read_index(iterator))
                            } else {
                                WordToken::StringMethod(method, variable, " ", Index::All)
                            };
                        }
                        self.read += 1;
                    }

                    panic!("ion: fatal error with syntax validation parsing: unterminated method");
                }
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    let variable = &self.data[start..self.read];

                    return if character == b'[' {
                        WordToken::Variable(variable, self.flags & DQUOTE != 0, self.read_index(iterator))
                    } else {
                        WordToken::Variable(variable, self.flags & DQUOTE != 0, Index::All)
                    };
                },
                _ => (),
            }
            self.read += 1;
        }

        WordToken::Variable(&self.data[start..], self.flags & DQUOTE != 0, Index::All)
    }

    fn read_index<I>(&mut self, iterator: &mut I) -> Index
        where I: Iterator<Item = u8>
    {
        self.read += 1;
        let start = self.read;
        while let Some(character) = iterator.next() {
            if let b']' = character {
                let index = match self.data[start..self.read].parse::<Index>() {
                    Ok(index) => index,
                    Err(_)    => Index::None
                };
                self.read += 1;
                return index
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
                    while let Some(character) = iterator.next() {
                        if character == b',' {
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
                                            method: method,
                                            variable: variable,
                                            pattern: Pattern::StringPattern(pattern),
                                            index: self.read_index(iterator)
                                        })
                                    } else {
                                        WordToken::ArrayMethod(ArrayMethod {
                                            method: method,
                                            variable: variable,
                                            pattern: Pattern::StringPattern(pattern),
                                            index: Index::All
                                        })
                                    }
                                }
                                self.read += 1;
                            }
                        } else if character == b')' {
                            // If no pattern is supplied, the default is a space.
                            let variable = &self.data[start..self.read];
                            self.read += 1;

                            return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                                let _ = iterator.next();
                                WordToken::ArrayMethod(ArrayMethod {
                                    method: method,
                                    variable: variable,
                                    pattern: Pattern::Whitespace,
                                    index: self.read_index(iterator)
                                })
                            } else {
                                WordToken::ArrayMethod(ArrayMethod {
                                    method: method,
                                    variable: variable,
                                    pattern: Pattern::Whitespace,
                                    index: Index::All
                                })
                            }
                        }
                        self.read += 1;
                    }

                    panic!("ion: fatal error with syntax validation parsing: unterminated method");
                },
                b'[' => {
                    return WordToken::ArrayVariable (
                        &self.data[start..self.read],
                        self.flags & DQUOTE != 0,
                        self.read_index(iterator)
                    );
                },
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    return WordToken::ArrayVariable(&self.data[start..self.read], self.flags & DQUOTE != 0, Index::All);
                },
                _ => (),
            }
            self.read += 1;
        }

        WordToken::ArrayVariable(&self.data[start..], self.flags & DQUOTE != 0, Index::All)
    }

    /// Contains the logic for parsing subshell syntax.
    fn process<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                b'$'  if self.flags & SQUOTE == 0 => {
                    if self.data.as_bytes()[self.read+1] == b'(' {
                        level += 1;
                    }
                },
                b')' if self.flags & SQUOTE == 0 => {
                    if level == 0 {
                        let output = &self.data[start..self.read];
                        self.read += 1;
                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::Process(
                                output,
                                self.flags & DQUOTE != 0,
                                self.read_index(iterator)
                            )
                        } else {
                            WordToken::Process(
                                output,
                                self.flags & DQUOTE != 0,
                                Index::All
                            )
                        }
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
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                b'@'  if self.flags & SQUOTE == 0 => {
                    if self.data.as_bytes()[self.read+1] == b'[' {
                        level += 1;
                    }
                },
                b']' if self.flags & SQUOTE == 0 => {
                    if level == 0 {
                        let array_process_contents = &self.data[start..self.read];
                        self.read += 1;
                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::ArrayProcess (
                                array_process_contents,
                                self.flags & DQUOTE != 0,
                                self.read_index(iterator)
                            )
                        } else {
                            WordToken::ArrayProcess (
                                array_process_contents,
                                self.flags & DQUOTE != 0,
                                Index::All
                            )
                        }
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
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                b','  if self.flags & (SQUOTE + DQUOTE) == 0 && level == 0 => {
                    elements.push(&self.data[start..self.read]);
                    start = self.read + 1;
                },
                b'{' if self.flags & (SQUOTE + DQUOTE) == 0 => level += 1,
                b'}' if self.flags & (SQUOTE + DQUOTE) == 0 => {
                    if level == 0 {
                        elements.push(&self.data[start..self.read]);
                        self.read += 1;
                        return WordToken::Brace(elements);
                    } else {
                        level -= 1;
                    }

                },
                _ => ()
            }
            self.read += 1;
        }

        panic!("ion: fatal error with syntax validation: unterminated brace")
    }

    /// Contains the grammar for parsing array expression syntax
    fn array<I>(&mut self, iterator: &mut I) -> WordToken<'a>
        where I: Iterator<Item = u8>
    {
        let mut start = self.read;
        let mut level = 0;
        let mut whitespace = true;
        let mut elements = Vec::new();
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                b' '  if self.flags & (SQUOTE + DQUOTE) == 0 && level == 0 => {
                    if whitespace {
                        self.read += 1;
                        start = self.read;
                    } else {
                        elements.push(&self.data[start..self.read]);
                        start = self.read + 1;
                        self.read += 1;
                        whitespace = true;
                    }
                    continue
                },
                b'[' if self.flags & (SQUOTE + DQUOTE) == 0 => level += 1,
                b']' if self.flags & (SQUOTE + DQUOTE) == 0 => {
                    if level == 0 {
                        elements.push(&self.data[start..self.read]);
                        self.read += 1;

                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::Array(elements, self.read_index(iterator))
                        } else {
                            WordToken::Array(elements, Index::All)

                        }
                    } else {
                        level -= 1;
                    }

                },
                _ => whitespace = false
            }
            self.read += 1;
        }

        panic!("ion: fatal error with syntax validation: unterminated array expression")
    }
}

impl<'a> Iterator for WordIterator<'a> {
    type Item = WordToken<'a>;

    fn next(&mut self) -> Option<WordToken<'a>> {
        if self.read == self.data.len() { return None }

        let mut iterator = self.data.bytes().skip(self.read);
        let mut start = self.read;

        loop {
            if let Some(character) = iterator.next() {
                match character {
                    _ if self.flags & BACKSL != 0 => {
                        self.read += 1;
                        self.flags ^= BACKSL;
                        break
                    },
                    b'\\' => {
                        start += 1;
                        self.read += 1;
                        self.flags ^= BACKSL;
                        if self.flags & EXPAND_PROCESSES == 0 {
                            return Some(WordToken::Normal("\\"));
                        }
                        break
                    }
                    b'\'' if self.flags & DQUOTE == 0 => {
                        start += 1;
                        self.read += 1;
                        self.flags ^= SQUOTE;
                        if self.flags & EXPAND_PROCESSES == 0 {
                            return Some(WordToken::Normal("'"));
                        }
                    },
                    b'"' if self.flags & SQUOTE == 0 => {
                        start += 1;
                        self.read += 1;
                        self.flags ^= DQUOTE;
                        if self.flags & EXPAND_PROCESSES == 0 {
                            return Some(WordToken::Normal("\""));
                        }
                    }
                    b' ' if self.flags & (SQUOTE + DQUOTE) == 0 => {
                        return Some(self.whitespaces(&mut iterator));
                    }
                    b'~' if self.flags & (SQUOTE + DQUOTE) == 0 => {
                        self.read += 1;
                        return Some(self.tilde(&mut iterator));
                    },
                    b'{' if self.flags & (SQUOTE + DQUOTE) == 0 => {
                        self.read += 1;
                        return Some(self.braces(&mut iterator));
                    },
                    b'[' if self.flags & SQUOTE == 0 => {
                        self.read += 1;
                        return Some(self.array(&mut iterator));
                    },
                    b'@' if self.flags & SQUOTE == 0 => {
                        match iterator.next() {
                            Some(b'[') => {
                                self.read += 2;
                                return if self.flags & EXPAND_PROCESSES != 0 {
                                    Some(self.array_process(&mut iterator))
                                } else {
                                    Some(WordToken::Normal(&self.data[start..self.read]))
                                }
                            },
                            // Some(b'{') => {
                            //     self.read += 2;
                            //     return Some(self.braced_variable(&mut iterator));
                            // }
                            _ => {
                                self.read += 1;
                                return Some(self.array_variable(&mut iterator));
                            }
                        }
                    }
                    b'$' if self.flags & SQUOTE == 0 => {
                        match iterator.next() {
                            Some(b'(') => {
                                self.read += 2;
                                return if self.flags & EXPAND_PROCESSES != 0 {
                                    Some(self.process(&mut iterator))
                                } else {
                                    Some(WordToken::Normal(&self.data[start..self.read]))
                                }
                            },
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
                    _ => { self.read += 1; break },
                }
            } else {
                return None
            }
        }

        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags & BACKSL != 0 => self.flags ^= BACKSL,
                b'\\' => {
                    self.flags ^= BACKSL;
                    let end = if self.flags & EXPAND_PROCESSES == 0 { self.read+1 } else { self.read };
                    let output = &self.data[start..end];
                    self.read += 1;
                    return Some(WordToken::Normal(output));
                },
                b'\'' if self.flags & DQUOTE == 0 => {
                    self.flags ^= SQUOTE;
                    let end = if self.flags & EXPAND_PROCESSES == 0 { self.read+1 } else { self.read };
                    let output = &self.data[start..end];
                    self.read += 1;
                    return Some(WordToken::Normal(output));
                },
                b'"' if self.flags & SQUOTE == 0 => {
                    self.flags ^= DQUOTE;
                    let end = if self.flags & EXPAND_PROCESSES == 0 { self.read+1 } else { self.read };
                    let output = &self.data[start..end];
                    self.read += 1;
                    return Some(WordToken::Normal(output));
                },
                b' ' | b'{' if self.flags & (SQUOTE + DQUOTE) == 0 => {
                    return Some(WordToken::Normal(&self.data[start..self.read]));
                },
                b'$' | b'@' | b'[' if self.flags & SQUOTE == 0 => {
                    return Some(WordToken::Normal(&self.data[start..self.read]));
                },
                _ => (),
            }
            self.read += 1;
        }

        if start == self.read {
            None
        } else {
            Some(WordToken::Normal(&self.data[start..]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare(input: &str, expected: Vec<WordToken>) {
        let mut correct = 0;
        for (actual, expected) in WordIterator::new(input, true).zip(expected.iter()) {
            assert_eq!(actual, *expected, "{:?} != {:?}", actual, expected);
            correct += 1;
        }
        assert_eq!(expected.len(), correct);
    }

    #[test]
    fn string_method() {
        let input = "$join(array, 'pattern') $join(array, 'pattern')";
        let expected = vec![
            WordToken::StringMethod("join", "array", "'pattern'", Index::All),
            WordToken::Whitespace(" "),
            WordToken::StringMethod("join", "array", "'pattern'", Index::All)
        ];
        compare(input, expected);
    }

    #[test]
    fn escape_with_backslash() {
        let input = "\\$FOO\\$BAR \\$FOO";
        let expected =
            vec![
                WordToken::Normal("$FOO"),
                WordToken::Normal("$BAR"),
                WordToken::Whitespace(" "),
                WordToken::Normal("$FOO")
            ];
        compare(input, expected);
    }

    #[test]
    fn array_expressions() {
        let input = "[ one two [three four]] [[one two] three four][0]";
        let first = vec![ "one", "two", "[three four]"];
        let second = vec![ "[one two]", "three", "four"];
        let expected = vec![
            WordToken::Array(first, Index::All),
            WordToken::Whitespace(" "),
            WordToken::Array(second, Index::ID(0)),
        ];
        compare(input, expected);
    }

    #[test]
    fn array_variables() {
        let input = "@array @array[0]";
        let expected = vec![
            WordToken::ArrayVariable("array", false, Index::All),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Index::ID(0)),
        ];
        compare(input, expected);
    }

    #[test]
    fn array_processes() {
        let input = "@[echo one two three] @[echo one two three][0]";
        let expected = vec![
            WordToken::ArrayProcess("echo one two three", false, Index::All),
            WordToken::Whitespace(" "),
            WordToken::ArrayProcess("echo one two three", false, Index::ID(0)),
        ];
        compare(input, expected);
    }

    #[test]
    fn indexes() {
        let input = "@array[0..3] @array[0...3] @array[abc] @array[..3] @array[3..]";
        let expected = vec![
            WordToken::ArrayVariable("array", false, Index::Range(IndexStart::new(0), IndexEnd::new(3))),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Index::Range(IndexStart::new(0), IndexEnd::FromEnd(4))),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Index::None),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Index::Range(IndexStart::new(0), IndexEnd::new(3))),
            WordToken::Whitespace(" "),
            WordToken::ArrayVariable("array", false, Index::Range(IndexStart::new(3), IndexEnd::CatchAll)),
        ];
        compare(input, expected);
    }

    #[test]
    fn nested_processes() {
        let input = "echo $(echo $(echo one)) $(echo one $(echo two) three)";
        let expected = vec![
            WordToken::Normal("echo"),
            WordToken::Whitespace(" "),
            WordToken::Process("echo $(echo one)", false, Index::All),
            WordToken::Whitespace(" "),
            WordToken::Process("echo one $(echo two) three", false, Index::All),
        ];
        compare(input, expected);
    }

    #[test]
    fn words_process_with_quotes() {
        let input = "echo $(git branch | rg '[*]' | awk '{print $2}')";
        let expected = vec![
            WordToken::Normal("echo"),
            WordToken::Whitespace(" "),
            WordToken::Process("git branch | rg '[*]' | awk '{print $2}'", false, Index::All),
        ];
        compare(input, expected);

        let input = "echo $(git branch | rg \"[*]\" | awk '{print $2}')";
        let expected = vec![
            WordToken::Normal("echo"),
            WordToken::Whitespace(" "),
            WordToken::Process("git branch | rg \"[*]\" | awk '{print $2}'", false, Index::All),
        ];
        compare(input, expected);
    }

    #[test]
    fn test_words() {
        let input = "echo $ABC \"${ABC}\" one{$ABC,$ABC} ~ $(echo foo) \"$(seq 1 100)\"";
        let expected = vec![
            WordToken::Normal("echo"),
            WordToken::Whitespace(" "),
            WordToken::Variable("ABC", false, Index::All),
            WordToken::Whitespace(" "),
            WordToken::Variable("ABC", true, Index::All),
            WordToken::Whitespace(" "),
            WordToken::Normal("one"),
            WordToken::Brace(vec!["$ABC", "$ABC"]),
            WordToken::Whitespace(" "),
            WordToken::Tilde("~"),
            WordToken::Whitespace(" "),
            WordToken::Process("echo foo", false, Index::All),
            WordToken::Whitespace(" "),
            WordToken::Process("seq 1 100", true, Index::All)
        ];
        compare(input, expected);
    }

    #[test]
    fn test_multiple_escapes() {
        let input = "foo\\(\\) bar\\(\\)";
        let expected = vec![
            WordToken::Normal("foo"),
            WordToken::Normal("("),
            WordToken::Normal(")"),
            WordToken::Whitespace(" "),
            WordToken::Normal("bar"),
            WordToken::Normal("("),
            WordToken::Normal(")"),
        ];
        compare(input, expected);
    }
}
