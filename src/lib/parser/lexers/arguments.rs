use err_derive::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comm {
    Type1,
    Type2,
    None,
}

/// A type of paired token
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Processes (ex: `$(..)`)
    Proc,
    /// Literal array (ex: `[ 1 .. 3 ]`)
    Array,
    /// Brace expansion (ex: `{a,b,c,d}`)
    Braces,
}
use self::Field::*;

/// The depth of various paired structures
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Levels {
    /// Parentheses
    parens: u8,
    /// Array literals
    array: u8,
    /// Braces
    braces: u8,
}

/// Error with paired tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
pub enum LevelsError {
    /// Unmatched opening parenthese
    #[error(display = "unmatched opening parenthese")]
    UnmatchedParen,
    /// Unmatched opening bracket
    #[error(display = "unmatched opening bracket")]
    UnmatchedBracket,
    /// Unmatched opening brace
    #[error(display = "unmatched opening brace")]
    UnmatchedBrace,
    /// Extra closing parenthese(s)
    #[error(display = "extra closing parenthese(s)")]
    ExtraParen,
    /// Extra closing bracket(s)
    #[error(display = "extra closing bracket(s)")]
    ExtraBracket,
    /// Extra closing brace(s)
    #[error(display = "extra closing brace(s)")]
    ExtraBrace,
}

impl Levels {
    /// Add a new depth level
    pub fn up(&mut self, field: Field) {
        let level = match field {
            Proc => &mut self.parens,
            Array => &mut self.array,
            Braces => &mut self.braces,
        };
        *level += 1;
    }

    /// Close paired tokens
    pub fn down(&mut self, field: Field) -> Result<(), LevelsError> {
        let level = match field {
            Proc if self.parens > 0 => &mut self.parens,
            Array if self.array > 0 => &mut self.array,
            Braces if self.braces > 0 => &mut self.braces,

            // errors
            Proc => return Err(LevelsError::ExtraParen),
            Array => return Err(LevelsError::ExtraBracket),
            Braces => return Err(LevelsError::ExtraBrace),
        };
        *level -= 1;
        Ok(())
    }

    /// Check if all parens where matched
    pub fn are_rooted(&self) -> bool { self.parens == 0 && self.array == 0 && self.braces == 0 }

    /// Check if all is ok
    pub fn check(&self) -> Result<(), LevelsError> {
        if self.parens > 0 {
            Err(LevelsError::UnmatchedParen)
        } else if self.array > 0 {
            Err(LevelsError::UnmatchedBracket)
        } else if self.braces > 0 {
            Err(LevelsError::UnmatchedBrace)
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
    comm: Comm,
    quotes: bool,
    variab: bool,
    array: bool,
    method: bool,
}

impl<'a> ArgumentSplitter<'a> {
    /// Create a new argument splitter based on the provided data
    pub fn new(data: &'a str) -> ArgumentSplitter<'a> {
        ArgumentSplitter {
            data,
            read: 0,
            comm: Comm::None,
            quotes: false,
            variab: false,
            array: false,
            method: false,
        }
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
                    self.array = true;
                    self.comm = Comm::Type2;
                    self.read += 1;
                    continue;
                }
                // Disable COMM_2 and enable COMM_1 + VARIAB.
                b'$' => {
                    self.variab = true;
                    self.comm = Comm::Type1;
                    self.read += 1;
                    continue;
                }
                b'[' => levels.up(Array),
                b']' => {
                    let _ = levels.down(Array);
                }
                b'{' => levels.up(Braces),
                b'}' => {
                    // TODO: handle errors here
                    let _ = levels.down(Braces);
                }
                b'(' => {
                    // Disable VARIAB + ARRAY and enable METHOD.
                    // if variab or array are set
                    if self.array || self.variab {
                        self.array = false;
                        self.variab = false;
                        self.method = true;
                    }
                    levels.up(Proc);
                }
                b')' => {
                    self.method = false;
                    let _ = levels.down(Proc);
                }

                // Toggle double quote rules.
                b'"' => {
                    self.quotes ^= true;
                }
                // Loop through characters until single quote rules are completed.
                b'\'' if !self.quotes => {
                    self.scan_singlequotes(&mut bytes);
                    self.read += 2;
                    continue;
                }
                // Break from the loop once a root-level space is found.
                b' ' => {
                    if !self.quotes && !self.method && levels.are_rooted() {
                        break;
                    }
                }
                _ => (),
            }

            self.read += 1;
            // disable COMM_1 and COMM_2
            self.comm = Comm::None;
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
