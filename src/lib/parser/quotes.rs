use std::{iter::Peekable, mem, str};

#[derive(Debug, PartialEq, Eq, Hash)]
enum Quotes {
    Single,
    Double,
    None,
}

/// Serves as a buffer for storing a string until that string can be terminated.
///
/// # Examples
///
/// This example comes from the shell's REPL, which ensures that the user's input
/// will only be submitted for execution once a terminated command is supplied.
#[derive(Debug)]
pub struct Terminator {
    buffer:     String,
    eof:        Option<String>,
    eof_buffer: String,
    array:      usize,
    read:       usize,
    trim:       bool,
    quotes:     Quotes,
}

impl<'a> From<&'a str> for Terminator {
    fn from(string: &'a str) -> Terminator { Terminator::new(string.to_owned()) }
}

impl From<String> for Terminator {
    fn from(string: String) -> Terminator { Terminator::new(string) }
}

#[derive(Debug)]
enum EarlyExit {
    Eof,
    Comment,
}

#[derive(Clone, Debug)]
pub struct RearPeekable<I: Iterator> {
    iter: Peekable<I>,
    now:  Option<I::Item>,
    last: Option<I::Item>,
}

impl<I> Iterator for RearPeekable<I>
where
    I: Iterator,
    I::Item: Copy,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        let next = self.iter.next();
        if next.is_some() {
            self.last = mem::replace(&mut self.now, next);
        }
        next
    }
}

impl<I: Iterator> RearPeekable<I> {
    #[inline]
    pub fn peek(&mut self) -> Option<&I::Item> { self.iter.peek() }

    #[inline]
    pub fn prev(&self) -> Option<&I::Item> { self.last.as_ref() }
}

impl Terminator {
    /// Consumes the `Terminator`, and returns the underlying `String`.
    pub fn consume(self) -> String { self.buffer }

    fn pair_components(&mut self) -> Option<EarlyExit> {
        let bytes = self
            .buffer
            .bytes()
            .enumerate()
            .skip(self.read)
            .peekable();

        let mut bytes = RearPeekable { iter: bytes, now: None, last: None };

        while let Some((i, character)) = bytes.next() {
            self.read = i + 1;

            if self.trim {
                self.trim = false;
            } else if character == b'\\' {
                self.trim = true;
            } else if self.quotes == Quotes::None {
                match character {
                    b'\'' => {
                        self.quotes = Quotes::Single;
                    }
                    b'"' => {
                        self.quotes = Quotes::Double;
                    }
                    b'<' if bytes.prev() == Some(&(i - 1, b'<')) => {
                        if bytes.peek() == Some(&(i + 1, b'<')) {
                            bytes.next();
                        } else {
                            let bytes = &self.buffer.as_bytes()[self.read..];
                            let eof_phrase = unsafe { str::from_utf8_unchecked(bytes) };
                            self.eof = Some(eof_phrase.trim().to_owned());
                            return Some(EarlyExit::Eof);
                        }
                    }
                    b'[' => {
                        self.array += 1;
                    }
                    b']' => {
                        if self.array > 0 {
                            self.array -= 1;
                        }
                    }
                    b'#' if bytes
                        .prev()
                        .filter(|&(j, c)| !(*j == i - 1 && [b' ', b'\n'].contains(c)))
                        .is_none() =>
                    {
                        return Some(EarlyExit::Comment);
                    }
                    _ => {},
                }
            } else {
                match (character, &self.quotes) {
                    (b'\'', Quotes::Single) | (b'"', Quotes::Double) => {
                        self.quotes = Quotes::None;
                    }
                    _ => (),
                }
            }
        }

        None
    }

    pub fn is_terminated(&mut self) -> bool {
        if self.eof.as_ref() == Some(&self.eof_buffer) {
            self.eof = None;
            self.buffer.push('\n');
            true
        } else if self.eof.is_some() {
            false
        } else {
            match self.pair_components() {
                Some(EarlyExit::Eof) => false,
                Some(EarlyExit::Comment) => {
                    self.buffer.truncate(self.read - 1);
                    self.array == 0 && self.quotes == Quotes::None
                }
                None => {
                    if let Some(b'\\') = self.buffer.bytes().last() {
                        self.buffer.pop();
                        self.read -= 1;
                        false
                    } else if self.array > 0 {
                        self.read += 1;
                        self.buffer.push(' ');
                        false
                    } else if self.quotes != Quotes::None {
                        self.read += 1;
                        self.buffer.push('\n');
                        false
                    } else {
                        let mut last_chars = self.buffer.bytes().rev();
                        last_chars
                            .next()
                            .filter(|&now| now == b'&' || now == b'|')
                            .and_then(|now| last_chars.next().filter(|&prev| prev == now))
                            .is_none()
                    }
                }
            }
        }
    }

    /// Appends a string to the internal buffer.
    pub fn append(&mut self, input: &str) {
        if self.eof.is_none() {
            self.buffer.push_str(if self.trim {
                self.trim = false;
                input.trim()
            } else {
                input
            });
        } else {
            self.eof_buffer.clear();
            self.eof_buffer.push_str(input.trim());
            self.buffer.push('\n');
            self.buffer.push_str(input);
        }
    }

    pub fn new(input: String) -> Terminator {
        Terminator {
            buffer:     input,
            eof:        None,
            eof_buffer: String::new(),
            array:      0,
            read:       0,
            trim:       false,
            quotes:     Quotes::None,
        }
    }
}
