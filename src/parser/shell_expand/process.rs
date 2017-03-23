/// Bit flags used by `VariableIterator`'s and `CommandExpander`'s flags fields.
const BACK:       u8 = 1;
const VAR_FOUND:  u8 = 2;
const SQUOTE:     u8 = 4;

/// Commands need to have variables expanded before they are submitted. This custom `Iterator` structure is
/// responsible for slicing the variables from within commands so that they can be expanded beforehand.
pub struct CommandExpander<'a> {
    data:  &'a str,
    read:  usize,
    flags: u8,
}

impl<'a> CommandExpander<'a> {
    pub fn new(data: &'a str) -> CommandExpander<'a> {
        CommandExpander { data: data, read: 0, flags: 0 }
    }
}

pub enum CommandToken<'a> {
    Normal(&'a str),
    Variable(&'a str),
}

impl<'a> Iterator for CommandExpander<'a> {
    type Item = CommandToken<'a>;

    fn next(&mut self) -> Option<CommandToken<'a>> {
        let start = self.read;
        let mut iterator = self.data.bytes().skip(self.read);

        while let Some(character) = iterator.next()  {
            self.read += 1;
            match character {
                b'\\' => { self.flags ^= BACK; continue },
                b'\'' if self.flags & BACK == 0 => self.flags ^= SQUOTE,
                b'$' if self.flags & (VAR_FOUND + BACK + SQUOTE) == 0 => {
                    if let Some(character) = self.data.bytes().nth(self.read) {
                        if character == b'(' { continue }
                    }

                    self.flags |= VAR_FOUND;
                    if start != self.read {
                        return Some(CommandToken::Normal(&self.data[start..self.read-1]));
                    }
                },
                _ if self.flags & VAR_FOUND != 0 => {
                    self.flags ^= VAR_FOUND;
                    if character == b'{' {
                        // Slice the braced variable from the command string.
                        while let Some(character) = iterator.next() {
                            self.read += 1;
                            match character {
                                b'\\' => { self.flags ^= BACK; self.read += 1; continue },
                                b'}' if self.flags & BACK == 0 => {
                                    return Some(CommandToken::Variable(&self.data[start+1..self.read-1]))
                                },
                                _ => ()
                            }
                            self.flags &= 255 ^ BACK;
                        }
                    } else {
                        // Slice the non-braced variable from the command string.
                        for character in iterator {
                            match character {
                                b'$' => {
                                    self.flags |= VAR_FOUND;
                                    self.read += 1;
                                    return Some(CommandToken::Variable(&self.data[start..self.read-1]))
                                }
                                b'{' | b'}' | b'(' | b')' | b' ' | b':' | b',' | b'@' | b'#' | b'\'' | b'"' =>
                                    return Some(CommandToken::Variable(&self.data[start..self.read])),
                                _ => ()
                            }
                            self.read += 1;
                        }

                        return Some(CommandToken::Variable(&self.data[start..self.read]));
                    }
                },
                _ => ()
            }
            self.flags &= 255 ^ BACK;
        }

        if start == self.read { None } else { Some(CommandToken::Normal(&self.data[start..])) }
    }
}

// TODO: Write Tests
