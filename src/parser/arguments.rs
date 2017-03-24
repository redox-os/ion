const DOUBLE: u8 = 1;
const SINGLE: u8 = 2;
const BACK:   u8 = 4;
const COMM:   u8 = 8;

// TODO: Handle Processes

/// An efficient `Iterator` structure for splitting arguments
pub struct ArgumentSplitter<'a> {
    buffer:       Vec<u8>,
    data:         &'a str,
    read:         usize,
    flags:        u8,
}

impl<'a> ArgumentSplitter<'a> {
    pub fn new(data: &'a str) -> ArgumentSplitter<'a> {
        ArgumentSplitter {
            buffer:       Vec::with_capacity(32),
            data:         data,
            read:         0,
            flags:        0,
        }
    }
}

impl<'a> Iterator for ArgumentSplitter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let mut level = 0;
        for character in self.data.bytes().skip(self.read) {
            self.read += 1;
            match character {
                _ if self.flags & BACK != 0 => self.flags ^= BACK,
                b'\\'                       => self.flags ^= BACK,
                b'$'  if self.flags & SINGLE == 0 => { self.flags |= COMM; self.buffer.push(character); continue },
                b'('  if self.flags & SINGLE == 0 && self.flags & COMM != 0 => level += 1,
                b')'  if self.flags & SINGLE == 0 => level -= 1,
                b'"'  if self.flags & SINGLE == 0 => self.flags ^= DOUBLE,
                b'\'' if self.flags & DOUBLE == 0 => self.flags ^= SINGLE,
                b' '  if !self.buffer.is_empty() && (self.flags & (SINGLE + DOUBLE) == 0) && level == 0 => break,
                _ => ()
            }
            self.buffer.push(character);
            self.flags &= 255 ^ COMM;
        }

        if self.buffer.is_empty() {
            None
        } else {
            let mut output = self.buffer.clone();
            output.shrink_to_fit();
            self.buffer.clear();
            Some(unsafe { String::from_utf8_unchecked(output) })
        }
    }
}
