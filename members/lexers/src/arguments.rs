bitflags! {
    struct ArgumentFlags: u8 {
        /// Double quotes
        const DOUBLE = 0b0000_0001;
        /// Command flags
        const COMM_1 = 0b0000_0010; // found $
        const COMM_2 = 0b0000_0100; // found ( after $
        /// String variable
        const VARIAB = 0b0000_1000;
        /// Array variable
        const ARRAY  = 0b0001_0000;
        const METHOD = 0b0010_0000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Proc,
    Array,
    Braces,
}
use self::Field::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Levels {
    parens: i32,
    array:  i32,
    braces: i32,
}

impl Levels {
    pub fn up(&mut self, field: Field) {
        let level = match field {
            Proc => &mut self.parens,
            Array => &mut self.array,
            Braces => &mut self.braces,
        };
        *level += 1;
    }

    pub fn down(&mut self, field: Field) {
        let level = match field {
            Proc => &mut self.parens,
            Array => &mut self.array,
            Braces => &mut self.braces,
        };
        *level -= 1;
    }

    pub fn are_rooted(&self) -> bool { self.parens == 0 && self.array == 0 && self.braces == 0 }

    pub fn check(&self) -> Result<(), &'static str> {
        if self.parens > 0 {
            Err("ion: syntax error: unmatched left paren")
        } else if self.array > 0 {
            Err("ion: syntax error: unmatched left bracket")
        } else if self.braces > 0 {
            Err("ion: syntax error: unmatched left brace")
        } else if self.parens < 0 {
            Err("ion: syntax error: extra right paren(s)")
        } else if self.array < 0 {
            Err("ion: syntax error: extra right bracket(s)")
        } else if self.braces < 0 {
            Err("ion: syntax error: extra right brace(s)")
        } else {
            Ok(())
        }
    }
}

/// An efficient `Iterator` structure for splitting arguments
#[derive(Debug)]
pub struct ArgumentSplitter<'a> {
    data: &'a str,
    /// Number of bytes read
    read: usize,
    bitflags: ArgumentFlags,
}

impl<'a> ArgumentSplitter<'a> {
    pub fn new(data: &'a str) -> ArgumentSplitter<'a> {
        ArgumentSplitter { data, read: 0, bitflags: ArgumentFlags::empty() }
    }

    fn scan_singlequotes<B: Iterator<Item = u8>>(&mut self, bytes: &mut B) {
        while let Some(character) = bytes.next() {
            match character {
                b'\\' => {
                    self.read += 2;
                    let _ = bytes.next();
                    continue;
                }
                b'\'' => break,
                _ => (),
            }
            self.read += 1;
        }
    }
}

impl<'a> Iterator for ArgumentSplitter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let data = self.data.as_bytes();
        while let Some(&b' ') = data.get(self.read) {
            self.read += 1;
        }
        let start = self.read;

        let mut levels = Levels::default();
        let mut bytes = data.iter().cloned().skip(self.read);
        while let Some(character) = bytes.next() {
            match character {
                // Skip the next byte.
                b'\\' => {
                    self.read += 2;
                    let _ = bytes.next();
                    continue;
                }
                // Disable COMM_1 and enable COMM_2 + ARRAY.
                b'@' => {
                    self.bitflags.remove(ArgumentFlags::COMM_1);
                    self.bitflags.insert(ArgumentFlags::COMM_2 | ArgumentFlags::ARRAY);
                    self.read += 1;
                    continue;
                }
                // Disable COMM_2 and enable COMM_1 + VARIAB.
                b'$' => {
                    self.bitflags.remove(ArgumentFlags::COMM_2);
                    self.bitflags.insert(ArgumentFlags::COMM_1 | ArgumentFlags::VARIAB);
                    self.read += 1;
                    continue;
                }
                b'[' => levels.up(Array),
                b']' => levels.down(Array),
                b'{' => levels.up(Braces),
                b'}' => levels.down(Braces),
                b'(' => {
                    // Disable VARIAB + ARRAY and enable METHOD.
                    // if variab or array are set
                    if self.bitflags.intersects(ArgumentFlags::VARIAB | ArgumentFlags::ARRAY) {
                        self.bitflags.remove(ArgumentFlags::VARIAB | ArgumentFlags::ARRAY);
                        self.bitflags.insert(ArgumentFlags::METHOD);
                    }
                    levels.up(Proc);
                }
                b')' => {
                    if self.bitflags.contains(ArgumentFlags::METHOD) {
                        self.bitflags.remove(ArgumentFlags::METHOD);
                    }
                    levels.down(Proc)
                }

                // Toggle double quote rules.
                b'"' => {
                    self.bitflags.toggle(ArgumentFlags::DOUBLE);
                }
                // Loop through characters until single quote rules are completed.
                b'\'' if !self.bitflags.contains(ArgumentFlags::DOUBLE) => {
                    self.scan_singlequotes(&mut bytes);
                    self.read += 2;
                    continue;
                }
                // Break from the loop once a root-level space is found.
                b' ' => {
                    if !self.bitflags.intersects(ArgumentFlags::DOUBLE | ArgumentFlags::METHOD)
                        && levels.are_rooted()
                    {
                        break;
                    }
                }
                _ => (),
            }

            self.read += 1;
            // disable COMM_1 and COMM_2
            self.bitflags.remove(ArgumentFlags::COMM_1 | ArgumentFlags::COMM_2);
        }

        if start == self.read {
            None
        } else {
            Some(&self.data[start..self.read])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare(input: &str, expected: Vec<&str>) {
        let arguments = ArgumentSplitter::new(input).collect::<Vec<&str>>();
        for (left, right) in expected.iter().zip(arguments.iter()) {
            assert_eq!(left, right);
        }
        assert_eq!(expected.len(), arguments.len());
    }

    #[test]
    fn methods() {
        let input = "echo $join(array, ', ') @split(var, ', ')";
        let expected = vec!["echo", "$join(array, ', ')", "@split(var, ', ')"];
        compare(input, expected);
    }

    #[test]
    fn processes() {
        let input = "echo $(echo one $(echo two)) @[echo one @[echo two]]";
        let expected = vec!["echo", "$(echo one $(echo two))", "@[echo one @[echo two]]"];
        compare(input, expected);
    }

    #[test]
    fn arrays() {
        let input = "echo [ one two @[echo three four] five ] [ six seven ]";
        let expected = vec!["echo", "[ one two @[echo three four] five ]", "[ six seven ]"];
        compare(input, expected);
    }

    #[test]
    fn quotes() {
        let input = "echo 'one two \"three four\"' \"five six 'seven eight'\"";
        let expected = vec!["echo", "'one two \"three four\"'", "\"five six 'seven eight'\""];
        compare(input, expected);
    }
}
