bitflags! {
    pub struct Flags : u8 {
        const BACKSL = 1;
        const SQUOTE = 2;
        const DQUOTE = 4;
        const TRIM   = 8;
    }
}


pub struct QuoteTerminator {
    buffer: String,
    read:   usize,
    flags:  Flags,
}

impl QuoteTerminator {
    pub fn new(input: String) -> QuoteTerminator {
        QuoteTerminator { buffer: input, read: 0, flags: Flags::empty() }
    }

    pub fn append(&mut self, input: String) {
        self.buffer.push_str(if self.flags.contains(TRIM) { input.trim() } else { &input });
    }

    pub fn check_termination(&mut self) -> bool {
        for character in self.buffer.bytes().skip(self.read) {
            self.read += 1;
            match character {
                _ if self.flags.contains(BACKSL)  => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if !self.flags.intersects(DQUOTE) => self.flags ^= SQUOTE,
                b'"'  if !self.flags.intersects(SQUOTE)  => self.flags ^= DQUOTE,
                _ => (),
            }
        }

        if self.flags.intersects(SQUOTE | DQUOTE) {
            self.read += 1;
            self.buffer.push('\n');
            false
        } else {
            match self.buffer.bytes().last() {
                Some(b'\\') => {
                    let _ = self.buffer.pop();
                    self.read -= 1;
                    self.flags |= TRIM;
                    false
                },
                Some(b'|') | Some(b'&') => {
                    // self.read -= 1;
                    // self.flags |= TRIM;
                    false
                }
                _ => true
            }
        }
    }

    pub fn consume(self) -> String { self.buffer }
}
