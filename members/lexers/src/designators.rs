use std::str;

bitflags! {
    struct Flags: u8 {
        const DQUOTE = 1;
        const SQUOTE = 2;
        const DESIGN = 4;
    }
}

#[derive(Debug)]
pub enum DesignatorToken<'a> {
    Designator(&'a str),
    Text(&'a str),
}

#[derive(Debug)]
pub struct DesignatorLexer<'a> {
    data:  &'a [u8],
    flags: Flags,
}

impl<'a> DesignatorLexer<'a> {
    fn grab_and_shorten(&mut self, id: usize) -> &'a str {
        let output = unsafe { str::from_utf8_unchecked(&self.data[..id]) };
        self.data = &self.data[id..];
        output
    }

    pub fn new(data: &'a [u8]) -> DesignatorLexer {
        DesignatorLexer { data, flags: Flags::empty() }
    }
}

impl<'a> Iterator for DesignatorLexer<'a> {
    type Item = DesignatorToken<'a>;

    fn next(&mut self) -> Option<DesignatorToken<'a>> {
        let mut iter = self.data.iter().enumerate();
        while let Some((id, byte)) = iter.next() {
            match *byte {
                b'\\' => {
                    let _ = iter.next();
                }
                b'"' if !self.flags.contains(Flags::SQUOTE) => self.flags ^= Flags::DQUOTE,
                b'\'' if !self.flags.contains(Flags::DQUOTE) => self.flags ^= Flags::SQUOTE,
                b'!' if !self.flags.intersects(Flags::DQUOTE | Flags::DESIGN) => {
                    self.flags |= Flags::DESIGN;
                    if id != 0 {
                        return Some(DesignatorToken::Text(self.grab_and_shorten(id)));
                    }
                }
                b' ' | b'\t' | b'\'' | b'"' | b'a'...b'z' | b'A'...b'Z'
                    if self.flags.contains(Flags::DESIGN) =>
                {
                    self.flags ^= Flags::DESIGN;
                    return Some(DesignatorToken::Designator(self.grab_and_shorten(id)));
                }
                _ => (),
            }
        }

        if self.data.is_empty() {
            None
        } else {
            let output = unsafe { str::from_utf8_unchecked(&self.data) };
            self.data = b"";
            Some(if self.flags.contains(Flags::DESIGN) {
                DesignatorToken::Designator(output)
            } else {
                DesignatorToken::Text(output)
            })
        }
    }
}
