use std::str;

bitflags! {
    pub struct Flags : u8 {
        const SQUOTE = 1;
        const DQUOTE = 2;
        const TRIM   = 4;
        const ARRAY  = 8;
        const COMM   = 16;
        const EOF    = 32;
    }
}

/// Serves as a buffer for storing a string until that string can be terminated.
///
/// # Examples
///
/// This example comes from the shell's REPL, which ensures that the user's input
/// will only be submitted for execution once a terminated command is supplied.
pub struct Terminator {
    buffer: String,
    eof: Option<String>,
    eof_buffer: String,
    array: usize,
    read: usize,
    flags: Flags,
}

impl<'a> From<&'a str> for Terminator {
    fn from(string: &'a str) -> Terminator {
        Terminator::new(string.to_owned())
    }
}

impl From<String> for Terminator {
    fn from(string: String) -> Terminator {
        Terminator::new(string)
    }
}

impl Terminator {
    pub fn new(input: String) -> Terminator {
        Terminator {
            buffer: input,
            eof: None,
            eof_buffer: String::new(),
            array: 0,
            read: 0,
            flags: Flags::empty(),
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
            self.eof_buffer.push_str(input);
        }
    }


    pub fn is_terminated(&mut self) -> bool {
        let mut eof_line = None;
        let eof = self.eof.clone();
        let status = if let Some(ref eof) = eof {
            let line = &self.eof_buffer;
            eof_line = Some([&line, "\n"].concat());
            line.trim() == eof
        } else {
            {
                let mut instance = Flags::empty();
                {
                    let mut bytes = self.buffer.bytes().skip(self.read);
                    while let Some(character) = bytes.next() {
                        self.read += 1;
                        match character {
                            b'\\' => {
                                let _ = bytes.next();
                            }
                            b'\'' if !self.flags.intersects(Flags::DQUOTE) => {
                                self.flags ^= Flags::SQUOTE
                            }
                            b'"' if !self.flags.intersects(Flags::SQUOTE) => {
                                self.flags ^= Flags::DQUOTE
                            }
                            b'<' if !self.flags.contains(Flags::SQUOTE | Flags::DQUOTE) => {
                                let as_bytes = self.buffer.as_bytes();
                                if Some(&b'<') == as_bytes.get(self.read) {
                                    self.read += 1;
                                    if Some(&b'<') != as_bytes.get(self.read) {
                                        let eof_phrase = unsafe {
                                            str::from_utf8_unchecked(&as_bytes[self.read..])
                                        };
                                        self.eof = Some(eof_phrase.trim().to_owned());
                                        instance |= Flags::EOF;
                                        break;
                                    }
                                }
                            }
                            b'[' if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) => {
                                self.flags |= Flags::ARRAY;
                                self.array += 1;
                            }
                            b']' if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) => {
                                self.array -= 1;
                                if self.array == 0 {
                                    self.flags -= Flags::ARRAY
                                }
                            }
                            b'#' if !self.flags.intersects(Flags::DQUOTE | Flags::SQUOTE) => {
                                if self.read > 1 {
                                    let character =
                                        self.buffer.as_bytes().get(self.read - 2).unwrap();
                                    if [b' ', b'\n'].contains(character) {
                                        instance |= Flags::COMM;
                                        break;
                                    }
                                } else {
                                    instance |= Flags::COMM;
                                    break;
                                }
                            }
                            _ => (),
                        }
                    }
                }
                if instance.contains(Flags::EOF) {
                    self.buffer.push('\n');
                    return false;
                } else if instance.contains(Flags::COMM) {
                    self.buffer.truncate(self.read - 1);
                    return !self.flags
                        .intersects(Flags::SQUOTE | Flags::DQUOTE | Flags::ARRAY);
                }
            }

            if self.flags
                .intersects(Flags::SQUOTE | Flags::DQUOTE | Flags::ARRAY)
            {
                if let Some(b'\\') = self.buffer.bytes().last() {
                    let _ = self.buffer.pop();
                    self.read -= 1;
                    self.flags |= Flags::TRIM;
                } else {
                    self.read += 1;
                    self.buffer.push(if self.flags.contains(Flags::ARRAY) {
                        ' '
                    } else {
                        '\n'
                    });
                }
                false
            } else {
                if let Some(b'\\') = self.buffer.bytes().last() {
                    let _ = self.buffer.pop();
                    self.read -= 1;
                    self.flags |= Flags::TRIM;
                    false
                } else {
                    // If the last two bytes are either '&&' or '||', we aren't terminated yet.
                    let bytes = self.buffer.as_bytes();
                    if bytes.len() >= 2 {
                        let bytes = &bytes[bytes.len() - 2..];
                        bytes != &[b'&', b'&'] && bytes != &[b'|', b'|']
                    } else {
                        true
                    }
                }
            }
        };

        if let Some(line) = eof_line {
            self.buffer.push_str(&line);
        }
        if self.eof.is_some() {
            self.eof_buffer.clear();
            if status {
                self.eof = None;
            }
        }
        status
    }

    /// Consumes the `Terminator`, and returns the underlying `String`.
    pub fn consume(self) -> String {
        self.buffer
    }
}
