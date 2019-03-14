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
pub struct Terminator<I: Iterator<Item = T>, T: AsRef<str>> {
    inner:      I,
    buffer:     String,
    eof:        Option<String>,
    array:      usize,
    read:       usize,
    trim:       bool,
    quotes:     Quotes,
    comment:    bool,
    terminated: bool,
}

impl<T: AsRef<str>> From<T> for Terminator<std::iter::Once<T>, T> {
    fn from(string: T) -> Self { Terminator::new(std::iter::once(string)) }
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

impl<I: Iterator<Item = T>, T: AsRef<str>> Terminator<I, T> {
    fn pair_components(&mut self) {
        let bytes = self
            .buffer
            .bytes()
            .enumerate()
            .skip(self.read)
            .peekable();

        let mut bytes = RearPeekable { iter: bytes, now: None, last: None };

        while let Some((i, character)) = bytes.next() {
            self.read = i + 1;

            if self.eof.is_some() {
            } else if self.comment && character == b'\n' {
                self.comment = false;
            } else if self.trim {
                self.trim = false;
            } else if character == b'\\' {
                self.trim = true;
            } else if self.quotes != Quotes::None {
                match (character, &self.quotes) {
                    (b'\'', Quotes::Single) | (b'"', Quotes::Double) => {
                        self.quotes = Quotes::None;
                    }
                    _ => (),
                }
            } else {
                match character {
                    b'\'' => {
                        self.quotes = Quotes::Single;
                    }
                    b'"' => {
                        self.quotes = Quotes::Double;
                    }
                    b'<' if bytes.prev() == Some(&(i - 1, b'<')) => {
                        if let Some(&(_, b'<')) = bytes.peek() {
                            bytes.next();
                        } else {
                            let bytes = &self.buffer.as_bytes()[self.read..];
                            let eof_phrase = unsafe { str::from_utf8_unchecked(bytes) };
                            self.eof = Some(eof_phrase.trim().to_owned());
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
                        .filter(|&(_, c)| ![b' ', b'\n'].contains(c))
                        .is_none() =>
                    {
                        self.comment = true;
                        // self.buffer.truncate(self.read - 1);
                    }
                    _ => {},
                }
            }

            let prev = bytes.prev().cloned();
            let next = bytes.peek();
            // println!("debug: \n\tnext: {:?}\n\tarray: {}\n\tquotes: {:?}\n\tcharacter: {:?}\n\tprev: {:?}\n\ttrim: {}", next, self.array, self.quotes, character as char, prev, self.trim);
            if (next == Some(&(i + 1, b'\n')) || next == None) &&
                !self.trim &&
                self.eof.is_none() &&
                self.array == 0 &&
                self.quotes == Quotes::None &&
                (![b'|', b'&'].contains(&character) || prev.filter(|&(_, c)| c == character).is_none()) {
                self.terminated = true;
                // println!("statement: {:?}", self.buffer);

                return;
            }
        }
    }

    /// Consumes lines until a statement is formed or the iterator runs dry, and returns the underlying `String`.
    pub fn terminate(mut self) -> Result<String, ()> {
        while !self.is_terminated() {
            if let Some(command) = self.inner.next() {
                self.append(command.as_ref());
            } else {
                return Err(());
            }
        }
        Ok(self.buffer.replace("\\\n", ""))
    }

    fn is_terminated(&mut self) -> bool {
        if !self.terminated {
            self.pair_components();
        }
        self.terminated
    }

    /// Appends a string to the internal buffer.
    fn append(&mut self, input: &str) {
        self.buffer.push(if self.array > 0 { ' ' } else { '\n' });
        self.buffer.push_str(if self.trim { input.trim_start() } else { input });

        if self.eof.as_ref().filter(|s| s.as_str() == input.trim()).is_some() {
            self.eof = None;
        }
    }

    pub fn new(inner: I) -> Terminator<I, T> {
        Terminator {
            inner,
            buffer:     String::new(),
            eof:        None,
            array:      0,
            read:       0,
            trim:       false,
            quotes:     Quotes::None,
            comment:    false,
            terminated: false,
        }
    }
}
