const BACKSL: u8 = 1;
const SQUOTE: u8 = 2;
const DQUOTE: u8 = 4;
const TRIM:   u8 = 8;

pub struct QuoteTerminator {
    buffer: String,
    read:   usize,
    flags:  u8,
}

impl QuoteTerminator {
    pub fn new(input: String) -> QuoteTerminator {
        QuoteTerminator { buffer: input, read: 0, flags: 0 }
    }

    pub fn append(&mut self, input: String) {
        self.buffer.push_str(if self.flags & TRIM != 0 { input.trim() } else { &input });
    }

    pub fn is_terminated(&mut self) -> bool {
        for character in self.buffer.bytes().skip(self.read) {
            self.read += 1;
            match character {
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                _ => (),
            }
        }

        if self.flags & (SQUOTE + DQUOTE) != 0 {
            self.read += 1;
            self.buffer.push('\n');
            false
        } else if self.buffer.bytes().last() == Some(b'\\') {
            let _ = self.buffer.pop();
            self.read -= 1;
            self.flags |= TRIM;
            false
        } else {
            true
        }
    }

    pub fn consume(self) -> String { self.buffer }
}