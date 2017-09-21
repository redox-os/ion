bitflags! {
    pub struct Flags : u8 {
        const BACKSL = 1;
        const SQUOTE = 2;
        const DQUOTE = 4;
        const TRIM   = 8;
    }
}

pub struct QuoteTerminator {
    buffer:     String,
    eof:        Option<String>,
    eof_buffer: String,
    read:       usize,
    flags:      Flags,
}

impl QuoteTerminator {
    pub fn new(input: String) -> QuoteTerminator {
        QuoteTerminator {
            buffer:     input,
            eof:        None,
            eof_buffer: String::new(),
            read:       0,
            flags:      Flags::empty(),
        }
    }

    pub fn append(&mut self, input: String) {
        if self.eof.is_none() {
            self.buffer.push_str(if self.flags.contains(TRIM) { input.trim() } else { &input });
        } else {
            self.eof_buffer.push_str(&input);
        }
    }

    pub fn check_termination(&mut self) -> bool {
        let mut eof_line = None;
        let eof = self.eof.clone();
        let status = if let Some(ref eof) = eof {
            let line = &self.eof_buffer;
            eof_line = Some([&line, "\n"].concat());
            line.trim() == eof
        } else {
            {
                let mut eof_found = false;
                {
                    let mut bytes = self.buffer.bytes().skip(self.read);
                    while let Some(character) = bytes.next() {
                        self.read += 1;
                        match character {
                            _ if self.flags.contains(BACKSL) => self.flags ^= BACKSL,
                            b'\\' => self.flags ^= BACKSL,
                            b'\'' if !self.flags.intersects(DQUOTE) => self.flags ^= SQUOTE,
                            b'"' if !self.flags.intersects(SQUOTE) => self.flags ^= DQUOTE,
                            b'<' if !self.flags.contains(SQUOTE | DQUOTE) => {
                                let as_bytes = self.buffer.as_bytes();
                                if Some(&b'<') == as_bytes.get(self.read) {
                                    self.read += 1;
                                    if Some(&b'<') != as_bytes.get(self.read) {
                                        use std::str;
                                        let eof_phrase = unsafe {
                                            str::from_utf8_unchecked(&as_bytes[self.read..])
                                        };
                                        self.eof = Some(eof_phrase.trim().to_owned());
                                        eof_found = true;
                                        break;
                                    }
                                }
                            }
                            _ => (),
                        }
                    }
                }
                if eof_found {
                    self.buffer.push('\n');
                    return false;
                }
            }

            if self.flags.intersects(SQUOTE | DQUOTE) {
                if let Some(b'\\') = self.buffer.bytes().last() {
                    let _ = self.buffer.pop();
                    self.read -= 1;
                    self.flags |= TRIM;
                } else {
                    self.read += 1;
                    self.buffer.push('\n');
                }
                false
            } else {
                if let Some(b'\\') = self.buffer.bytes().last() {
                    let _ = self.buffer.pop();
                    self.read -= 1;
                    self.flags |= TRIM;
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

    pub fn consume(self) -> String { self.buffer }
}
