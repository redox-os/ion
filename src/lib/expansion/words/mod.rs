#[cfg(test)]
mod tests;

use super::methods::{ArrayMethod, Pattern, StringMethod};
use crate::parser::lexers::ArgumentSplitter;
pub use crate::ranges::{Select, SelectWithSize};
use std::borrow::Cow;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
enum Quotes {
    None,
    Single,
    Double,
}

/// Unescapes filenames to be passed into the completer
pub fn unescape(input: &str) -> Cow<'_, str> {
    let mut input: Cow<'_, str> = input.into();
    while let Some(found) = input.find('\\') {
        if input.as_ref().len() > found + 1 {
            input.to_mut().remove(found);
        } else {
            break;
        }
    }
    input
}

pub fn unescape_characters<'a>(input: &'a str, characters: &[char]) -> Cow<'a, str> {
    let mut last_idx: usize = 0;
    let mut input: Cow<'_, str> = input.into();

    while let Some(idx) = input[last_idx..].find('\\') {
        if let Some(next_character) = input.chars().nth(last_idx + idx + 1) {
            if characters.contains(&next_character) {
                input.to_mut().remove(last_idx + idx);
            }
        }
        last_idx += idx + 1;
    }

    input
}

fn index_until_unescaped_character(input: &str, characters: &[u8]) -> (usize, u8, u8, bool) {
    let mut i: usize = 0;
    let mut prev_character = b'\0';
    let mut last_character = b'\0';
    let mut glob_character_found = false;
    let mut backslash = false;

    for byte in input.bytes() {
        if backslash {
            // Skip over escaped character
            backslash = false;
        } else if byte == b'\\' {
            backslash = true;
        } else {
            if !glob_character_found && [b'?', b'*'].contains(&byte) {
                glob_character_found = true;
            }

            if characters.contains(&byte) {
                last_character = byte;
                break;
            } else {
                prev_character = byte;
            }
        }

        i += 1;
    }

    (i, prev_character, last_character, glob_character_found)
}

fn index_until_character(input: &str, characters: &[u8], ret_on_match: bool) -> (usize, u8) {
    let mut i: usize = 0;
    let mut last_character = b'\0';

    for byte in input.bytes() {
        last_character = byte;
        if ret_on_match ^ characters.contains(&byte) {
            i += 1;
        } else {
            break;
        }
    }

    (i, last_character)
}

/// Terminal tokens for a Ion script
#[derive(Debug, PartialEq, Clone)]
pub enum WordToken<'a> {
    /// Represents a normal string who may contain a globbing character
    /// (the second element) or a tilde expression (the third element)
    Normal(Cow<'a, str>, bool, bool),
    /// Whitespace
    Whitespace(&'a str),
    /// Braced alternatives
    Brace(Vec<&'a str>),
    /// An array literal
    Array(Vec<&'a str>, Option<&'a str>),
    /// A scalar variable
    Variable(&'a str, Option<&'a str>),
    /// An array or map-like variable
    ArrayVariable(&'a str, bool, Option<&'a str>),
    /// A process that should expand to an array
    ArrayProcess(&'a str, bool, Option<&'a str>),
    /// A process that expands to a scalar value
    Process(&'a str, Option<&'a str>),
    /// A method on a scalar value
    StringMethod(StringMethod<'a>),
    /// A method on a array value
    ArrayMethod(ArrayMethod<'a>, bool),
    /// An arithmetic expression
    Arithmetic(&'a str),
}

/// Iterate over the terminal tokens of the parsed text
#[derive(Debug, PartialEq, Clone)]
pub struct WordIterator<'a> {
    data:    &'a str,
    read:    usize,
    quotes:  Quotes,
    backsl:  bool,
    do_glob: bool,
}

impl<'a> WordIterator<'a> {
    fn arithmetic_expression<I: Iterator<Item = u8>>(&mut self, iter: &mut I) -> WordToken<'a> {
        let _ = iter.next();

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

    fn glob_check<I>(&mut self, iterator: &mut I, is_text_adjacent: bool) -> bool
    where
        I: Iterator<Item = u8> + Clone,
    {
        // Clone the iterator and scan for illegal characters until the corresponding ]
        // is discovered. If none are found, then it's a valid glob signature.
        let mut moves = 0;
        let mut glob = false;
        let mut square_bracket = 0;
        let mut iter = iterator.clone().peekable();

        while let Some(character) = iter.next() {
            moves += 1;
            match character {
                b'[' => {
                    square_bracket += 1;
                }
                b' ' | b'"' | b'\'' | b'$' | b'{' | b'}' => break,
                b']' => {
                    // If the glob is less than three bytes in width, then it's empty and thus
                    // invalid. If it's not adjacent to text, it's not a glob.
                    let next_char = iter.peek();
                    if (moves >= 2 && square_bracket == 0)
                        && (is_text_adjacent || next_char != None && next_char != Some(&b' '))
                    {
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
            // self.read += moves + 1;
            self.read += moves;
            true
        } else {
            self.read += 1;
            false
        }
    }

    /// Contains the grammar for parsing array expression syntax
    fn array<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.backsl => self.backsl = false,
                b'\\' => self.backsl = true,
                b'\'' if self.quotes == Quotes::Single => self.quotes = Quotes::None,
                b'\'' if self.quotes == Quotes::None => self.quotes = Quotes::Single,
                b'"' if self.quotes == Quotes::Double => self.quotes = Quotes::None,
                b'"' if self.quotes == Quotes::None => self.quotes = Quotes::Double,
                b'[' if self.quotes == Quotes::None => level += 1,
                b']' if self.quotes == Quotes::None => {
                    if level == 0 {
                        let elements =
                            ArgumentSplitter::new(&self.data[start..self.read]).collect::<Vec<_>>();
                        self.read += 1;

                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::Array(elements, Some(self.read_selection(iterator)))
                        } else {
                            WordToken::Array(elements, None)
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

    /// Contains the grammar for parsing brace expansion syntax
    fn braces<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let mut start = self.read;
        let mut level = 0;
        let mut elements = Vec::new();
        for character in iterator {
            match character {
                _ if self.backsl => self.backsl = false,
                b'\\' => self.backsl = true,
                b'\'' if self.quotes == Quotes::Single => self.quotes = Quotes::None,
                b'\'' if self.quotes == Quotes::None => self.quotes = Quotes::Single,
                b'"' if self.quotes == Quotes::Double => self.quotes = Quotes::None,
                b'"' if self.quotes == Quotes::None => self.quotes = Quotes::Double,
                b',' if self.quotes == Quotes::None && level == 0 => {
                    elements.push(&self.data[start..self.read]);
                    start = self.read + 1;
                }
                b'{' if self.quotes == Quotes::None => level += 1,
                b'}' if self.quotes == Quotes::None => {
                    if level == 0 {
                        elements.push(&self.data[start..self.read]);
                        self.read += 1;
                        return WordToken::Brace(elements);
                    } else {
                        level -= 1;
                    }
                }
                b'[' if self.quotes == Quotes::None => level += 1,
                b']' if self.quotes == Quotes::None => level -= 1,
                _ => (),
            }
            self.read += 1;
        }

        panic!("ion: fatal error with syntax validation: unterminated brace")
    }

    /// Contains the logic for parsing array subshell syntax.
    fn array_process<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let _ = iterator.next();
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.backsl => self.backsl = false,
                b'\\' => self.backsl = true,
                b'\'' if self.quotes == Quotes::Single => self.quotes = Quotes::None,
                b'\'' if self.quotes == Quotes::None => self.quotes = Quotes::Single,
                b'"' if self.quotes == Quotes::Double => self.quotes = Quotes::None,
                b'"' if self.quotes == Quotes::None => self.quotes = Quotes::Double,
                b'@' if self.quotes != Quotes::Single => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        level += 1;
                    }
                }
                b')' if self.quotes != Quotes::Single => {
                    if level == 0 {
                        let array_process_contents = &self.data[start..self.read];
                        self.read += 1;
                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::ArrayProcess(
                                array_process_contents,
                                self.quotes == Quotes::Double,
                                Some(self.read_selection(iterator)),
                            )
                        } else {
                            WordToken::ArrayProcess(
                                array_process_contents,
                                self.quotes == Quotes::Double,
                                None,
                            )
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

    /// Contains the logic for parsing subshell syntax.
    fn process<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.backsl => self.backsl = false,
                b'\\' => self.backsl = true,
                b'\'' if self.quotes == Quotes::Single => self.quotes = Quotes::None,
                b'\'' if self.quotes == Quotes::None => self.quotes = Quotes::Single,
                b'"' if self.quotes == Quotes::Double => self.quotes = Quotes::None,
                b'"' if self.quotes == Quotes::None => self.quotes = Quotes::Double,
                b'$' if self.quotes != Quotes::Single => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        // Pop out the '(' char
                        iterator.next();
                        self.read += 1;
                        level += 1;
                    }
                }
                b'@' if self.quotes != Quotes::Single => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        // Pop out the '(' char
                        iterator.next();
                        self.read += 1;
                        level += 1;
                    }
                }
                b')' if self.quotes != Quotes::Single => {
                    if level == 0 {
                        let output = &self.data[start..self.read];
                        self.read += 1;
                        return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                            let _ = iterator.next();
                            WordToken::Process(output, Some(self.read_selection(iterator)))
                        } else {
                            WordToken::Process(output, None)
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

    fn braced_array_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let _ = iterator.next();
        let start = self.read;
        // self.read += 1;
        while let Some(character) = iterator.next() {
            match character {
                b'[' => {
                    let result = WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.quotes == Quotes::Double,
                        Some(self.read_selection(iterator)),
                    );
                    self.read += 1;
                    if let Some(b'}') = iterator.next() {
                        return result;
                    }
                    panic!(
                        "ion: fatal with syntax validation error: unterminated braced array \
                         expression"
                    );
                }
                b'}' => {
                    let output = &self.data[start..self.read];
                    self.read += 1;
                    return WordToken::ArrayVariable(output, self.quotes == Quotes::Double, None);
                }
                // Only alphanumerical and underscores are allowed in variable names
                0..=47 | 58..=64 | 91..=94 | 96 | 123..=127 => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.quotes == Quotes::Double,
                        None,
                    );
                }
                _ => (),
            }
            self.read += 1;
        }
        WordToken::ArrayVariable(&self.data[start..], self.quotes == Quotes::Double, None)
    }

    /// Contains the logic for parsing array variable syntax
    fn array_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let mut method_flags = Quotes::None;
        let mut start = self.read;
        // self.read += 1;
        while let Some(character) = iterator.next() {
            match character {
                b'(' => {
                    let method = &self.data[start..self.read];
                    self.read += 1;
                    start = self.read;
                    let mut depth = 0;
                    while let Some(character) = iterator.next() {
                        match character {
                            b'\'' if method_flags == Quotes::Single => method_flags = Quotes::None,
                            b'\'' if method_flags == Quotes::None => method_flags = Quotes::Single,
                            b'"' if method_flags == Quotes::Double => method_flags = Quotes::None,
                            b'"' if method_flags == Quotes::None => method_flags = Quotes::Double,
                            b'[' if method_flags == Quotes::None => depth += 1,
                            b']' if method_flags == Quotes::None => depth -= 1,
                            b' ' if depth == 0 && method_flags == Quotes::None => {
                                let variable = &self.data[start..self.read];
                                self.read += 1;
                                start = self.read;
                                while let Some(character) = iterator.next() {
                                    if character == b')' {
                                        let pattern = &self.data[start..self.read].trim();
                                        self.read += 1;
                                        return if let Some(&b'[') =
                                            self.data.as_bytes().get(self.read)
                                        {
                                            let _ = iterator.next();
                                            WordToken::ArrayMethod(
                                                ArrayMethod::new(
                                                    method,
                                                    variable.trim(),
                                                    Pattern::StringPattern(pattern),
                                                    Some(self.read_selection(iterator)),
                                                ),
                                                self.quotes == Quotes::Double,
                                            )
                                        } else {
                                            WordToken::ArrayMethod(
                                                ArrayMethod::new(
                                                    method,
                                                    variable.trim(),
                                                    Pattern::StringPattern(pattern),
                                                    None,
                                                ),
                                                self.quotes == Quotes::Double,
                                            )
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
                                    WordToken::ArrayMethod(
                                        ArrayMethod::new(
                                            method,
                                            variable.trim(),
                                            Pattern::Whitespace,
                                            Some(self.read_selection(iterator)),
                                        ),
                                        self.quotes == Quotes::Double,
                                    )
                                } else {
                                    WordToken::ArrayMethod(
                                        ArrayMethod::new(
                                            method,
                                            variable.trim(),
                                            Pattern::Whitespace,
                                            None,
                                        ),
                                        self.quotes == Quotes::Double,
                                    )
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
                        self.quotes == Quotes::Double,
                        Some(self.read_selection(iterator)),
                    );
                }
                // Only alphanumerical and underscores are allowed in variable names
                0..=47 | 58..=64 | 91..=94 | 96 | 123..=127 => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.quotes == Quotes::Double,
                        None,
                    );
                }
                _ => (),
            }
            self.read += 1;
        }

        WordToken::ArrayVariable(&self.data[start..], self.quotes == Quotes::Double, None)
    }

    fn read_selection<I>(&mut self, iterator: &mut I) -> &'a str
    where
        I: Iterator<Item = u8>,
    {
        self.read += 1;
        let start = self.read;
        for character in iterator {
            if let b']' = character {
                let value = &self.data[start..self.read];
                self.read += 1;
                return value;
            }
            self.read += 1;
        }

        panic!()
    }

    /// Contains the logic for parsing variable syntax
    fn variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let mut method_flags = Quotes::None;
        let mut start = self.read;
        while let Some(character) = iterator.next() {
            match character {
                b'(' => {
                    let method = &self.data[start..self.read];
                    self.read += 1;
                    start = self.read;
                    let mut depth = 0;
                    while let Some(character) = iterator.next() {
                        match character {
                            b'\'' if method_flags == Quotes::Single => method_flags = Quotes::None,
                            b'\'' if method_flags == Quotes::None => method_flags = Quotes::Single,
                            b'"' if method_flags == Quotes::Double => method_flags = Quotes::None,
                            b'"' if method_flags == Quotes::None => method_flags = Quotes::Double,
                            b'[' if method_flags == Quotes::None => depth += 1,
                            b']' if method_flags == Quotes::None => depth -= 1,
                            b' ' if depth == 0 && method_flags == Quotes::None => {
                                let variable = &self.data[start..self.read];
                                self.read += 1;
                                start = self.read;
                                while let Some(character) = iterator.next() {
                                    if character == b')' {
                                        self.read += 1;
                                        if depth != 0 {
                                            depth -= 1;
                                            continue;
                                        }
                                        let pattern = &self.data[start..self.read - 1].trim();
                                        return if let Some(&b'[') =
                                            self.data.as_bytes().get(self.read)
                                        {
                                            let _ = iterator.next();
                                            WordToken::StringMethod(StringMethod {
                                                method,
                                                variable: variable.trim(),
                                                pattern,
                                                selection: Some(self.read_selection(iterator)),
                                            })
                                        } else {
                                            WordToken::StringMethod(StringMethod {
                                                method,
                                                variable: variable.trim(),
                                                pattern,
                                                selection: None,
                                            })
                                        };
                                    } else if character == b'(' {
                                        depth += 1;
                                    } else if character == b'\\' {
                                        self.read += 1;
                                        let _ = iterator.next();
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
                                        selection: Some(self.read_selection(iterator)),
                                    })
                                } else {
                                    WordToken::StringMethod(StringMethod {
                                        method,
                                        variable: variable.trim(),
                                        pattern: " ",
                                        selection: None,
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
                0..=47 | 58..=64 | 91..=94 | 96 | 123..=127 => {
                    let variable = &self.data[start..self.read];

                    return if character == b'[' {
                        WordToken::Variable(variable, Some(self.read_selection(iterator)))
                    } else {
                        WordToken::Variable(variable, None)
                    };
                }
                _ => (),
            }
            self.read += 1;
        }

        WordToken::Variable(&self.data[start..], None)
    }

    // Contains the logic for parsing braced variables
    fn braced_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let _ = iterator.next();
        let start = self.read;
        for character in iterator {
            if character == b'}' {
                let output = &self.data[start..self.read];
                self.read += 1;
                return WordToken::Variable(output, None);
            }
            self.read += 1;
        }

        // The validator at the frontend should catch unterminated braced variables.
        panic!("ion: fatal error with syntax validation parsing: unterminated braced variable");
    }

    /// Creates a new iterator with a given expander
    pub const fn new(data: &'a str, do_glob: bool) -> WordIterator<'a> {
        WordIterator { data, backsl: false, read: 0, quotes: Quotes::None, do_glob }
    }
}

impl<'a> Iterator for WordIterator<'a> {
    type Item = WordToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.read == self.data.len() {
            return None;
        }

        let mut iterator = self.data.bytes().skip(self.read).peekable();
        let mut start = self.read;
        let mut glob = false;
        let mut tilde = false;
        let mut looped = false;

        loop {
            let character = iterator.next()?;
            match character {
                b'\'' => {
                    match self.quotes {
                        Quotes::None => {
                            start += 1;
                            self.read += 1;

                            let (idx, _) =
                                index_until_character(&self.data[start..], &[b'\''], true);
                            self.read += idx;

                            // Do we care if there is no matching single quote? This case is already
                            // handled by src/lib/parser/pipelines.rs
                            let ret = Some(WordToken::Normal(
                                self.data[start..self.read].into(),
                                glob,
                                tilde,
                            ));
                            self.read += 1;
                            return ret;
                        }
                        Quotes::Double => {
                            self.read += 1;
                            return Some(WordToken::Normal(
                                self.data[start..self.read].into(),
                                glob,
                                tilde,
                            ));
                        }
                        Quotes::Single => {
                            // Should never happen
                            panic!();
                        }
                    }
                }
                b'"' => {
                    match self.quotes {
                        Quotes::None => {
                            start += 1;
                            self.read += 1;
                            self.quotes = Quotes::Double;

                            let peeked_character = iterator.peek();
                            if peeked_character == Some(&b'"') {
                                self.read += 1;
                                self.quotes = Quotes::None;
                                return Some(WordToken::Normal("".into(), glob, tilde));
                            }
                        }
                        Quotes::Double => {
                            start += 1;
                            self.read += 1;
                            self.quotes = Quotes::None;
                        }
                        Quotes::Single => {
                            // Should never happen
                            panic!();
                        }
                    }
                }
                b'$' => {
                    match self.quotes {
                        Quotes::None | Quotes::Double => {
                            self.read += 1;
                            let peeked_character1 = iterator.peek();
                            match peeked_character1 {
                                Some(b'(') => {
                                    let _ = iterator.next();
                                    self.read += 1;
                                    let peeked_character2 = iterator.peek();
                                    if peeked_character2 == Some(&b'(') {
                                        self.read += 1;
                                        return Some(self.arithmetic_expression(&mut iterator));
                                    } else {
                                        return Some(self.process(&mut iterator));
                                    }
                                }
                                Some(b'{') => {
                                    self.read += 1;
                                    return Some(self.braced_variable(&mut iterator));
                                }
                                Some(b' ') => {
                                    return Some(WordToken::Normal(
                                        self.data[start..self.read].into(),
                                        glob,
                                        tilde,
                                    ))
                                }
                                Some(b'?') => {
                                    start += 1;
                                    self.read += 1;
                                    return Some(WordToken::Variable(
                                        self.data[start..self.read].into(),
                                        None,
                                    ));
                                }
                                _ => return Some(self.variable(&mut iterator)),
                            }
                        }
                        Quotes::Single => {
                            // Should never happen
                            panic!();
                        }
                    }
                }
                b'@' => {
                    match self.quotes {
                        Quotes::None | Quotes::Double => {
                            self.read += 1;
                            let peeked_character1 = iterator.peek();

                            match peeked_character1 {
                                Some(b'(') => {
                                    self.read += 1;
                                    return Some(self.array_process(&mut iterator));
                                }
                                Some(b'{') => {
                                    self.read += 1;
                                    return Some(self.braced_array_variable(&mut iterator));
                                }
                                Some(b' ') => {
                                    return Some(WordToken::Normal(
                                        self.data[start..self.read].into(),
                                        glob,
                                        tilde,
                                    ))
                                }
                                _ => return Some(self.array_variable(&mut iterator)),
                            }
                        }
                        Quotes::Single => {
                            // Should never happen
                            panic!();
                        }
                    }
                }
                b'{' => match self.quotes {
                    Quotes::None => {
                        self.read += 1;
                        return Some(self.braces(&mut iterator));
                    }
                    Quotes::Single | Quotes::Double => {
                        self.read += 1;
                        return Some(WordToken::Normal(
                            self.data[start..self.read].into(),
                            glob,
                            tilde,
                        ));
                    }
                },
                b'[' => match self.quotes {
                    Quotes::None => {
                        if self.glob_check(&mut iterator, false) {
                            glob = self.do_glob;
                            looped = true;
                            continue;
                        } else {
                            return Some(self.array(&mut iterator));
                        }
                    }
                    Quotes::Single | Quotes::Double => {
                        self.read += 1;
                        return Some(WordToken::Normal(
                            self.data[start..self.read].into(),
                            glob,
                            tilde,
                        ));
                    }
                },
                b'~' => {
                    if self.quotes != Quotes::Single {
                        self.read += 1;
                        tilde = true;
                        return Some(WordToken::Normal(
                            self.data[start..self.read].into(),
                            glob,
                            tilde,
                        ));
                    }
                }
                b' ' => {
                    let (idx, _) = index_until_character(&self.data[start..], &[b' '], false);
                    self.read += idx;
                    return Some(WordToken::Whitespace(self.data[start..self.read].into()));
                }
                _ => {
                    let (idx, prev_character, last_character, glob_character_found) =
                        match self.quotes {
                            Quotes::None | Quotes::Single => {
                                if glob {
                                    index_until_unescaped_character(
                                        &self.data[self.read..],
                                        &[b' ', b'\'', b'"', b'$', b'@', b'~', b'{'],
                                    )
                                } else if looped {
                                    index_until_unescaped_character(
                                        &self.data[self.read..],
                                        &[b' ', b'\'', b'"', b'$', b'@', b'~', b'{', b'['],
                                    )
                                } else {
                                    index_until_unescaped_character(
                                        &self.data[start..],
                                        &[b' ', b'\'', b'"', b'$', b'@', b'~', b'{', b'['],
                                    )
                                }
                            }
                            Quotes::Double => {
                                if glob {
                                    index_until_unescaped_character(
                                        &self.data[self.read..],
                                        &[b' ', b'\'', b'"', b'$', b'@', b'~'],
                                    )
                                } else {
                                    index_until_unescaped_character(
                                        &self.data[start..],
                                        &[b' ', b'\'', b'"', b'$', b'@', b'~'],
                                    )
                                }
                            }
                        };

                    if idx > 0 || looped {
                        self.read += idx;
                        looped = false;

                        match last_character {
                            b'[' => {
                                // Point self.read to the character after the square bracket [
                                self.read += 1;

                                if prev_character != b'=' {
                                    for _ in 0..idx {
                                        // Advance the iterator for side-effects
                                        iterator.next();
                                    }

                                    if self.do_glob
                                        && self.quotes == Quotes::None
                                        && glob_character_found
                                    {
                                        glob = self.do_glob;

                                        // Even though we've found a glob character we want to call
                                        // glob_check for side-effects to iterator and self.read
                                        if self.glob_check(&mut iterator, true) {
                                            if iterator.peek() != None {
                                                // We haven't reached the end of the iterator yet
                                                // For example *werty[abc]efg
                                                continue;
                                            } else {
                                                // We've reached the end of the iterator
                                                // For example *werty[abc]
                                                // Yes, this branch does nothing
                                            }
                                        } else {
                                            // We've already found a glob character but we've also
                                            // found an invalid glob with square brackets
                                            // For example *werty[] or *werty[ abc]
                                            // Rewind self.read by 2 so we can read up to but not
                                            // including the square bracket [
                                            // and treat whatever comes after the end of our word as
                                            // part of a new word token
                                            self.read -= 2;
                                        }
                                    } else if self.glob_check(&mut iterator, true) {
                                        // We've found a valid glob with square brackets
                                        glob = self.do_glob;
                                        if iterator.peek() != None {
                                            // We haven't reached the end of the iterator yet
                                            // For example werty[abc]efg
                                            looped = true;
                                            continue;
                                        } else {
                                            // We've reached the end of the iterator
                                            // For example werty[abc]
                                            // Yes, this branch does nothing
                                        }
                                    } else {
                                        // We've found an invalid glob with square brackets
                                        // For example werty[] or werty[ abc]
                                        // Rewind self.read by 2 so we can read up to but not
                                        // including the square bracket [
                                        // and treat whatever comes after the end of our word as
                                        // part of a new word token
                                        self.read -= 2;
                                    }
                                } else {
                                    // Handles the corner case of let map:hmap[[int]] = [key1=[1 2 3
                                    // 4 5] key2=[6 7 8]]
                                    // This branch will result in the word token key1=[ being
                                    // returned Yes, this branch
                                    // does nothing
                                }
                            }
                            _ => {
                                if self.do_glob
                                    && self.quotes == Quotes::None
                                    && glob_character_found
                                {
                                    glob = self.do_glob;
                                }
                            }
                        }

                        let output = self.data[start..self.read].into();
                        let output = unescape_characters(
                            output,
                            &[' ', '\'', '"', '$', '@', '~', '?', '*', '{', '(', ')', '}', '\\'],
                        );
                        return Some(WordToken::Normal(output, glob, tilde));
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}
