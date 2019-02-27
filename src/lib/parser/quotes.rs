use itertools::Itertools;
use std::{iter::Peekable, mem, str};

bitflags! {
    pub struct Flags : u8 {
        const SQUOTE = 1;
        const DQUOTE = 2;
        const TRIM   = 4;
    }
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
    flags:      Flags,
}

impl<'a> From<&'a str> for Terminator {
    fn from(string: &'a str) -> Terminator { Terminator::new(string.to_owned()) }
}

impl From<String> for Terminator {
    fn from(string: String) -> Terminator { Terminator::new(string) }
}

#[derive(Debug)]
enum NotTerminatedErr {
    StartEof,
    Eof,
    Comment,
    UnclosedArray,
    UnclosedString,
    EscapedNewline,
    AndOrClause,
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

    #[inline]
    pub fn now(&self) -> Option<&I::Item> { self.now.as_ref() }
}

impl Terminator {
    /// Consumes the `Terminator`, and returns the underlying `String`.
    pub fn consume(self) -> String { self.buffer }

    fn pair_components(&mut self) -> Result<(), NotTerminatedErr> {
        if self.eof.as_ref() == Some(&self.eof_buffer) {
            return Err(NotTerminatedErr::StartEof);
        } else if self.eof.is_some() {
            return Err(NotTerminatedErr::Eof);
        }

        let bytes = self
            .buffer
            .bytes()
            .enumerate()
            .skip(self.read)
            .coalesce(
                |prev, next| {
                    if prev.1 == b'\\' {
                        Ok((next.0, 0))
                    } else {
                        Err((prev, next))
                    }
                },
            )
            .filter(|&(_, c)| c != 0)
            //.inspect(|c| println!("{:?} {}", c.0, c.1 as char))
            .peekable();

        let mut bytes = RearPeekable { iter: bytes, now: None, last: None };

        while let Some((i, character)) = bytes.next() {
            self.read = i + 1;

            match character {
                b'\'' if !self.flags.intersects(Flags::DQUOTE) => {
                    if bytes.find(|&(_, c)| c == b'\'').is_none() {
                        self.flags ^= Flags::SQUOTE;
                    }
                }
                b'"' if !self.flags.intersects(Flags::SQUOTE) => {
                    if bytes.find(|&(_, c)| c == b'"').is_none() {
                        self.flags ^= Flags::DQUOTE;
                    }
                }
                b'<' if bytes.prev() == Some(&(i - 1, b'<')) => {
                    if bytes.peek() == Some(&(i + 1, b'<')) {
                        bytes.next();
                    } else {
                        let bytes = &self.buffer.as_bytes()[self.read..];
                        let eof_phrase = unsafe { str::from_utf8_unchecked(bytes) };
                        self.eof = Some(eof_phrase.trim().to_owned());
                        return Err(NotTerminatedErr::Eof);
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
                    return Err(NotTerminatedErr::Comment);
                }
                _ => (),
            }
        }

        if let Some((_, b'\\')) = bytes.now() {
            Err(NotTerminatedErr::EscapedNewline)
        } else if self.array > 0 {
            Err(NotTerminatedErr::UnclosedArray)
        } else if self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE) {
            Err(NotTerminatedErr::UnclosedString)
        } else if bytes
            .now()
            .filter(|&&(_, now)| now == b'&' || now == b'|')
            .and_then(|&(_, now)| bytes.prev().filter(|&&(_, prev)| prev == now))
            .is_some()
        {
            Err(NotTerminatedErr::AndOrClause)
        } else {
            Ok(())
        }
    }

    pub fn is_terminated(&mut self) -> bool {
        match self.pair_components() {
            Err(NotTerminatedErr::StartEof) => {
                self.eof = None;
                self.buffer.push('\n');
                true
            }
            Err(NotTerminatedErr::Eof) => false,
            Err(NotTerminatedErr::Comment) => {
                self.buffer.truncate(self.read - 1);
                self.array == 0 && !self.flags.intersects(Flags::SQUOTE | Flags::DQUOTE)
            }
            Err(NotTerminatedErr::EscapedNewline) => {
                self.buffer.pop();
                self.read -= 1;
                self.flags = Flags::TRIM;
                false
            }
            Err(NotTerminatedErr::UnclosedString) => {
                self.read += 1;
                self.buffer.push('\n');
                false
            }
            Err(NotTerminatedErr::UnclosedArray) => {
                self.read += 1;
                self.buffer.push(' ');
                false
            }
            Err(NotTerminatedErr::AndOrClause) => false,
            Ok(()) => true,
        }
    }

    /// Appends a string to the internal buffer.
    pub fn append(&mut self, input: &str) {
        if self.eof.is_none() {
            self.buffer.push_str(if self.flags.contains(Flags::TRIM) {
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
            flags:      Flags::empty(),
        }
    }
}
