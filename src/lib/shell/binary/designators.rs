use parser::ArgumentSplitter;
use shell::Shell;
use std::{borrow::Cow, str};

bitflags! {
    struct Flags: u8 {
        const DQUOTE = 1;
        const SQUOTE = 2;
        const DESIGN = 4;
    }
}

#[derive(Debug)]
enum Token<'a> {
    Designator(&'a str),
    Text(&'a str),
}

struct DesignatorSearcher<'a> {
    data:  &'a [u8],
    flags: Flags,
}

impl<'a> DesignatorSearcher<'a> {
    fn grab_and_shorten(&mut self, id: usize) -> &'a str {
        let output = unsafe { str::from_utf8_unchecked(&self.data[..id]) };
        self.data = &self.data[id..];
        output
    }

    fn new(data: &'a [u8]) -> DesignatorSearcher {
        DesignatorSearcher {
            data,
            flags: Flags::empty(),
        }
    }
}

impl<'a> Iterator for DesignatorSearcher<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
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
                        return Some(Token::Text(self.grab_and_shorten(id)));
                    }
                }
                b' ' | b'\t' | b'\'' | b'"' | b'a'...b'z' | b'A'...b'Z'
                    if self.flags.contains(Flags::DESIGN) =>
                {
                    self.flags ^= Flags::DESIGN;
                    return Some(Token::Designator(self.grab_and_shorten(id)));
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
                Token::Designator(output)
            } else {
                Token::Text(output)
            })
        }
    }
}

pub(crate) fn expand_designators<'a>(shell: &Shell, cmd: &'a str) -> Cow<'a, str> {
    let context = shell.context.as_ref().unwrap();
    if let Some(buffer) = context.history.buffers.iter().last() {
        let buffer = buffer.as_bytes();
        let buffer = unsafe { str::from_utf8_unchecked(&buffer) };
        let mut output = String::with_capacity(cmd.len());
        for token in DesignatorSearcher::new(cmd.as_bytes()) {
            match token {
                Token::Text(text) => output.push_str(text),
                Token::Designator(text) => match text {
                    "!!" => output.push_str(buffer),
                    "!$" => output.push_str(last_arg(buffer)),
                    "!0" => output.push_str(command(buffer)),
                    "!^" => output.push_str(first_arg(buffer)),
                    "!*" => output.push_str(&args(buffer)),
                    _ => output.push_str(text),
                },
            }
        }
        return Cow::Owned(output);
    }

    Cow::Borrowed(cmd)
}

fn command<'a>(text: &'a str) -> &'a str { ArgumentSplitter::new(text).next().unwrap_or(text) }

fn args(text: &str) -> &str {
    let bytes = text.as_bytes();
    bytes.iter()
        // Obtain position of the first space character,
        .position(|&x| x == b' ')
        // and then obtain the arguments to the command.
        .and_then(|fp| bytes[fp+1..].iter()
            // Find the position of the first character in the first argument.
            .position(|&x| x != b' ')
            // Then slice the argument string from the original command.
            .map(|sp| &text[fp+sp+1..]))
        // Unwrap the arguments string if it exists, else return the original string.
        .unwrap_or(text)
}

fn first_arg<'a>(text: &'a str) -> &'a str { ArgumentSplitter::new(text).nth(1).unwrap_or(text) }

fn last_arg<'a>(text: &'a str) -> &'a str { ArgumentSplitter::new(text).last().unwrap_or(text) }
