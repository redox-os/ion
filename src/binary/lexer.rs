use std::str;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Quotes {
    Double,
    Single,
    None,
}

#[derive(Debug)]
pub enum DesignatorToken<'a> {
    Designator(&'a str),
    Text(&'a str),
}

#[derive(Debug)]
pub struct DesignatorLexer<'a> {
    data:   &'a [u8],
    quotes: Quotes,
    design: bool,
}

impl<'a> DesignatorLexer<'a> {
    fn grab_and_shorten(&mut self, id: usize) -> &'a str {
        let output = unsafe { str::from_utf8_unchecked(&self.data[..id]) };
        self.data = &self.data[id..];
        output
    }

    pub fn new(data: &'a [u8]) -> DesignatorLexer {
        DesignatorLexer { data, quotes: Quotes::None, design: false }
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
                b'"' if self.quotes == Quotes::None => self.quotes = Quotes::Double,
                b'"' if self.quotes == Quotes::Double => self.quotes = Quotes::None,
                b'\'' if self.quotes == Quotes::None => self.quotes = Quotes::Single,
                b'\'' if self.quotes == Quotes::Single => self.quotes = Quotes::None,
                b'!' if self.quotes != Quotes::Double && !self.design => {
                    self.design = true;
                    if id != 0 {
                        return Some(DesignatorToken::Text(self.grab_and_shorten(id)));
                    }
                }
                b' ' | b'\t' | b'\'' | b'"' | b'a'..=b'z' | b'A'..=b'Z' if self.design => {
                    self.design = false;
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
            Some(if self.design {
                DesignatorToken::Designator(output)
            } else {
                DesignatorToken::Text(output)
            })
        }
    }
}
