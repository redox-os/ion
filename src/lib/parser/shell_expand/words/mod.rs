mod index;
mod methods;
mod range;
mod select;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod benchmarks;

#[cfg(test)]
pub(crate) use self::methods::Key;
pub(crate) use self::{
    index::Index, methods::{ArrayMethod, Pattern, StringMethod}, range::Range,
    select::{Select, SelectWithSize},
};
use super::{super::ArgumentSplitter, expand_string, Expander};
use shell::escape::unescape;
use std::borrow::Cow;

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
    }
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum WordToken<'a> {
    /// Represents a normal string who may contain a globbing character
    /// (the second element) or a tilde expression (the third element)
    Normal(Cow<'a, str>, bool, bool),
    Whitespace(&'a str),
    Brace(Vec<&'a str>),
    Array(Vec<&'a str>, Select),
    Variable(&'a str, bool, Select),
    ArrayVariable(&'a str, bool, Select),
    ArrayProcess(&'a str, bool, Select),
    Process(&'a str, bool, Select),
    StringMethod(StringMethod<'a>),
    ArrayMethod(ArrayMethod<'a>),
    Arithmetic(&'a str),
}

#[derive(Debug)]
pub(crate) struct WordIterator<'a, E: Expander + 'a> {
    data:      &'a str,
    read:      usize,
    flags:     Flags,
    expanders: &'a E,
    do_glob: bool,
}

impl<'a, E: Expander + 'a> WordIterator<'a, E> {
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

    fn glob_check<I>(&mut self, iterator: &mut I) -> bool
    where
        I: Iterator<Item = u8> + Clone,
    {
        // Clone the iterator and scan for illegal characters until the corresponding ]
        // is discovered. If none are found, then it's a valid glob signature.
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
                b' ' | b'"' | b'\'' | b'$' | b'{' | b'}' => break,
                b']' => {
                    // If the glob is less than three bytes in width, then it's empty and thus
                    // invalid. If it's not adjacent to text, it's not a glob.
                    let next_char = iter.clone().next();
                    if !(moves <= 3 && square_bracket == 1)
                        && (next_char != None && next_char != Some(b' '))
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
            self.read += moves + 1;
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
                _ if self.flags.contains(Flags::BACKSL) => self.flags ^= Flags::BACKSL,
                b'\\' => self.flags ^= Flags::BACKSL,
                b'\'' if !self.flags.contains(Flags::DQUOTE) => self.flags ^= Flags::SQUOTE,
                b'"' if !self.flags.contains(Flags::SQUOTE) => self.flags ^= Flags::DQUOTE,
                b'[' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => level += 1,
                b']' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => if level == 0 {
                    let elements =
                        ArgumentSplitter::new(&self.data[start..self.read]).collect::<Vec<&str>>();
                    self.read += 1;

                    return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                        let _ = iterator.next();
                        WordToken::Array(elements, self.read_selection(iterator))
                    } else {
                        WordToken::Array(elements, Select::All)
                    };
                } else {
                    level -= 1;
                },
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
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(Flags::BACKSL) => self.flags ^= Flags::BACKSL,
                b'\\' => self.flags ^= Flags::BACKSL,
                b'\'' if !self.flags.contains(Flags::DQUOTE) => self.flags ^= Flags::SQUOTE,
                b'"' if !self.flags.contains(Flags::SQUOTE) => self.flags ^= Flags::DQUOTE,
                b',' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) && level == 0 => {
                    elements.push(&self.data[start..self.read]);
                    start = self.read + 1;
                },
                b'{' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => level += 1,
                b'}' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => if level == 0 {
                    elements.push(&self.data[start..self.read]);
                    self.read += 1;
                    return WordToken::Brace(elements);
                } else {
                    level -= 1;
                },
                b'[' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => level += 1,
                b']' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => level -= 1,
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
        let start = self.read;
        let mut level = 0;
        while let Some(character) = iterator.next() {
            match character {
                _ if self.flags.contains(Flags::BACKSL) => self.flags ^= Flags::BACKSL,
                b'\\' => self.flags ^= Flags::BACKSL,
                b'\'' if !self.flags.contains(Flags::DQUOTE) => self.flags ^= Flags::SQUOTE,
                b'"' if !self.flags.contains(Flags::SQUOTE) => self.flags ^= Flags::DQUOTE,
                b'@' if !self.flags.contains(Flags::SQUOTE) => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        level += 1;
                    }
                }
                b')' if !self.flags.contains(Flags::SQUOTE) => if level == 0 {
                    let array_process_contents = &self.data[start..self.read];
                    self.read += 1;
                    return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                        let _ = iterator.next();
                        WordToken::ArrayProcess(
                            array_process_contents,
                            self.flags.contains(Flags::DQUOTE),
                            self.read_selection(iterator),
                        )
                    } else {
                        WordToken::ArrayProcess(
                            array_process_contents,
                            self.flags.contains(Flags::DQUOTE),
                            Select::All,
                        )
                    };
                } else {
                    level -= 1;
                },
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
                _ if self.flags.contains(Flags::BACKSL) => self.flags ^= Flags::BACKSL,
                b'\\' => self.flags ^= Flags::BACKSL,
                b'\'' if !self.flags.contains(Flags::DQUOTE) => self.flags ^= Flags::SQUOTE,
                b'"' if !self.flags.contains(Flags::SQUOTE) => self.flags ^= Flags::DQUOTE,
                b'$' if !self.flags.contains(Flags::SQUOTE) => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        // Pop out the '(' char
                        iterator.next();
                        self.read += 1;
                        level += 1;
                    }
                }
                b'@' if !self.flags.contains(Flags::SQUOTE) => {
                    if self.data.as_bytes()[self.read + 1] == b'(' {
                        // Pop out the '(' char
                        iterator.next();
                        self.read += 1;
                        level += 1;
                    }
                }
                b')' if !self.flags.contains(Flags::SQUOTE) => if level == 0 {
                    let output = &self.data[start..self.read];
                    self.read += 1;
                    return if let Some(&b'[') = self.data.as_bytes().get(self.read) {
                        let _ = iterator.next();
                        WordToken::Process(
                            output,
                            self.flags.contains(Flags::DQUOTE),
                            self.read_selection(iterator),
                        )
                    } else {
                        WordToken::Process(output, self.flags.contains(Flags::DQUOTE), Select::All)
                    };
                } else {
                    level -= 1;
                },
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
        let start = self.read;
        // self.read += 1;
        while let Some(character) = iterator.next() {
            match character {
                b'[' => {
                    let result = WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(Flags::DQUOTE),
                        self.read_selection(iterator),
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
                    return WordToken::ArrayVariable(
                        output,
                        self.flags.contains(Flags::DQUOTE),
                        Select::All,
                    );
                }
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(Flags::DQUOTE),
                        Select::All,
                    )
                }
                _ => (),
            }
            self.read += 1;
        }
        WordToken::ArrayVariable(
            &self.data[start..],
            self.flags.contains(Flags::DQUOTE),
            Select::All,
        )
    }

    /// Contains the logic for parsing array variable syntax
    fn array_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
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
                                        return if let Some(&b'[') =
                                            self.data.as_bytes().get(self.read)
                                        {
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
                        self.flags.contains(Flags::DQUOTE),
                        self.read_selection(iterator),
                    )
                }
                // Only alphanumerical and underscores are allowed in variable names
                0...47 | 58...64 | 91...94 | 96 | 123...127 => {
                    return WordToken::ArrayVariable(
                        &self.data[start..self.read],
                        self.flags.contains(Flags::DQUOTE),
                        Select::All,
                    )
                }
                _ => (),
            }
            self.read += 1;
        }

        WordToken::ArrayVariable(
            &self.data[start..],
            self.flags.contains(Flags::DQUOTE),
            Select::All,
        )
    }

    fn read_selection<I>(&mut self, iterator: &mut I) -> Select
    where
        I: Iterator<Item = u8>,
    {
        self.read += 1;
        let start = self.read;
        while let Some(character) = iterator.next() {
            if let b']' = character {
                let value =
                    expand_string(&self.data[start..self.read], self.expanders, false).join(" ");
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

    /// Contains the logic for parsing variable syntax
    fn variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
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
                        WordToken::Variable(
                            variable,
                            self.flags.contains(Flags::DQUOTE),
                            self.read_selection(iterator),
                        )
                    } else {
                        WordToken::Variable(
                            variable,
                            self.flags.contains(Flags::DQUOTE),
                            Select::All,
                        )
                    };
                }
                _ => (),
            }
            self.read += 1;
        }

        WordToken::Variable(
            &self.data[start..],
            self.flags.contains(Flags::DQUOTE),
            Select::All,
        )
    }

    // Contains the logic for parsing braced variables
    fn braced_variable<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
    {
        let start = self.read;
        while let Some(character) = iterator.next() {
            if character == b'}' {
                let output = &self.data[start..self.read];
                self.read += 1;
                return WordToken::Variable(output, self.flags.contains(Flags::DQUOTE), Select::All);
            }
            self.read += 1;
        }

        // The validator at the frontend should catch unterminated braced variables.
        panic!("ion: fatal error with syntax validation parsing: unterminated braced variable");
    }

    // Contains the grammar for collecting whitespace characters
    fn whitespaces<I>(&mut self, iterator: &mut I) -> WordToken<'a>
    where
        I: Iterator<Item = u8>,
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

    pub(crate) fn new(data: &'a str, expanders: &'a E, do_glob: bool) -> WordIterator<'a, E> {
        WordIterator {
            data,
            read: 0,
            flags: Flags::empty(),
            expanders,
            do_glob,
        }
    }
}

impl<'a, E: Expander + 'a> Iterator for WordIterator<'a, E> {
    type Item = WordToken<'a>;

    fn next(&mut self) -> Option<WordToken<'a>> {
        if self.read == self.data.len() {
            return None;
        }

        let mut iterator = self.data.bytes().skip(self.read).peekable();
        let mut start = self.read;
        let mut glob = false;
        let mut tilde = false;

        loop {
            if let Some(character) = iterator.next() {
                match character {
                    _ if self.flags.contains(Flags::BACKSL) => {
                        self.read += 1;
                        self.flags ^= Flags::BACKSL;
                        break;
                    }
                    b'\\' => {
                        if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) {
                            start += 1;
                        }
                        self.read += 1;
                        self.flags ^= Flags::BACKSL;
                        break;
                    }
                    b'\'' if !self.flags.contains(Flags::DQUOTE) => {
                        start += 1;
                        self.read += 1;
                        self.flags ^= Flags::SQUOTE;
                        break;
                    }
                    b'"' if !self.flags.contains(Flags::SQUOTE) => {
                        start += 1;
                        self.read += 1;
                        if self.flags.contains(Flags::DQUOTE) {
                            self.flags -= Flags::DQUOTE;
                            return self.next();
                        }
                        self.flags |= Flags::DQUOTE;
                        break;
                    }
                    b' ' if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) => {
                        return Some(self.whitespaces(&mut iterator))
                    }
                    b'~' if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) => {
                        tilde = true;
                        self.read += 1;
                        break;
                    }
                    b'{' if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) => {
                        self.read += 1;
                        return Some(self.braces(&mut iterator));
                    }
                    b'[' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => {
                        if self.glob_check(&mut iterator) {
                            glob = self.do_glob;
                        } else {
                            return Some(self.array(&mut iterator));
                        }
                    }
                    b'@' if !self.flags.contains(Flags::SQUOTE) => match iterator.next() {
                        Some(b'(') => {
                            self.read += 2;
                            return Some(self.array_process(&mut iterator));
                        }
                        Some(b'{') => {
                            self.read += 2;
                            return Some(self.braced_array_variable(&mut iterator));
                        }
                        Some(b' ') | None => {
                            self.read += 1;
                            let output = &self.data[start..self.read];
                            return Some(WordToken::Normal(output.into(), glob, tilde));
                        }
                        _ => {
                            self.read += 1;
                            return Some(self.array_variable(&mut iterator));
                        }
                    },
                    b'$' if !self.flags.contains(Flags::SQUOTE) => {
                        match iterator.next() {
                            Some(b'(') => {
                                self.read += 2;
                                return if self.data.as_bytes()[self.read] == b'(' {
                                    // Pop the incoming left paren
                                    let _ = iterator.next();
                                    self.read += 1;
                                    Some(self.arithmetic_expression(&mut iterator))
                                } else {
                                    Some(self.process(&mut iterator))
                                };
                            }
                            Some(b'{') => {
                                self.read += 2;
                                return Some(self.braced_variable(&mut iterator));
                            }
                            Some(b' ') | None => {
                                self.read += 1;
                                let output = &self.data[start..self.read];
                                return Some(WordToken::Normal(output.into(), glob, tilde));
                            }
                            _ => {
                                self.read += 1;
                                return Some(self.variable(&mut iterator));
                            }
                        }
                    }
                    b'*' | b'?' => {
                        self.read += 1;
                        glob = self.do_glob;
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
                _ if self.flags.contains(Flags::BACKSL) => self.flags ^= Flags::BACKSL,
                b'\\' if !self.flags.contains(Flags::SQUOTE) => {
                    pub(crate) fn maybe_unescape(input: &str, contains_escapeable: bool) -> Cow<str> {
                        if !contains_escapeable {
                            input.into()
                        } else {
                            unescape(input)
                        }
                    }

                    let next = iterator.next();
                    self.read += 1;

                    if self.flags.contains(Flags::DQUOTE) {
                        let _ = iterator.next();
                        self.read += 1;
                        return Some(WordToken::Normal(maybe_unescape(&self.data[start..self.read], next.map_or(true, |c| c == b'$' || c == b'@' || c == b'\\' || c == b'"')), glob, tilde));
                    }
                }
                b'\'' if !self.flags.contains(Flags::DQUOTE) => {
                    self.flags ^= Flags::SQUOTE;
                    let output = &self.data[start..self.read];
                    self.read += 1;
                    return Some(WordToken::Normal(output.into(), glob, tilde));
                }
                b'"' if !self.flags.contains(Flags::SQUOTE) => {
                    self.flags ^= Flags::DQUOTE;
                    let output = &self.data[start..self.read];
                    self.read += 1;
                    return Some(WordToken::Normal(output.into(), glob, tilde));
                }
                b' ' | b'{' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => {
                    return Some(WordToken::Normal(unescape(&self.data[start..self.read]), glob, tilde))
                }
                b'$' | b'@' if !self.flags.contains(Flags::SQUOTE) => {
                    if let Some(&character) = self.data.as_bytes().get(self.read) {
                        if character == b' ' {
                            self.read += 1;
                            let output = &self.data[start..self.read];
                            return Some(WordToken::Normal(output.into(), glob, tilde));
                        }
                    }
                    let output = &self.data[start..self.read];
                    if output != "" {
                        return Some(WordToken::Normal(unescape(output), glob, tilde));
                    } else {
                        return self.next();
                    };
                }
                b'[' if !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) => {
                    if self.glob_check(&mut iterator) {
                        glob = self.do_glob;
                    } else {
                        return Some(WordToken::Normal(self.data[start..self.read].into(), glob, tilde));
                    }
                }
                b'*' | b'?' if !self.flags.contains(Flags::SQUOTE) => {
                    glob = self.do_glob;
                }
                _ => (),
            }
            self.read += 1;
        }

        if start == self.read {
            None
        } else {
            Some(WordToken::Normal(unescape(&self.data[start..]), glob, tilde))
        }
    }
}
