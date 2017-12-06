const DOUBLE: u8 = 1;
const COMM_1: u8 = 2;
const COMM_2: u8 = 4;
const VARIAB: u8 = 8;
const ARRAY: u8 = 16;
const METHOD: u8 = 32;

/// An efficient `Iterator` structure for splitting arguments
pub struct ArgumentSplitter<'a> {
    data: &'a str,
    read: usize,
    flags: u8,
}

impl<'a> ArgumentSplitter<'a> {
    pub fn new(data: &'a str) -> ArgumentSplitter<'a> {
        ArgumentSplitter {
            data: data,
            read: 0,
            flags: 0,
        }
    }
}

impl<'a> ArgumentSplitter<'a> {
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

        let (mut level, mut alevel) = (0, 0);
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
                    self.flags = (self.flags & (255 ^ COMM_1)) | (COMM_2 + ARRAY);
                    self.read += 1;
                    continue;
                }
                // Disable COMM_2 and enable COMM_1 + VARIAB.
                b'$' => {
                    self.flags = (self.flags & (255 ^ COMM_2)) | (COMM_1 + VARIAB);
                    self.read += 1;
                    continue;
                }
                // Increment the array level
                b'[' => alevel += 1,
                // Decrement the array level
                b']' => alevel -= 1,
                // Increment the parenthesis level.
                b'(' if self.flags & COMM_1 != 0 => level += 1,
                // Disable VARIAB + ARRAY and enable METHOD.
                b'(' if self.flags & (VARIAB + ARRAY) != 0 => {
                    self.flags = (self.flags & (255 ^ (VARIAB + ARRAY))) | METHOD;
                }
                // Disable METHOD if enabled.
                b')' if self.flags & METHOD != 0 => self.flags ^= METHOD,
                // Otherwise decrement the parenthesis level.
                b')' => level -= 1,
                // Toggle double quote rules.
                b'"' => self.flags ^= DOUBLE,
                // Loop through characters until single quote rules are completed.
                b'\'' if self.flags & DOUBLE == 0 => {
                    self.scan_singlequotes(&mut bytes);
                    self.read += 2;
                    continue;
                }
                // Break from the loop once a root-level space is found.
                b' ' if (self.flags & (DOUBLE + METHOD)) + level + alevel == 0 => break,
                _ => (),
            }
            self.read += 1;
            self.flags &= 255 ^ (COMM_1 + COMM_2);
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
        let expected = vec![
            "echo",
            "[ one two @[echo three four] five ]",
            "[ six seven ]",
        ];
        compare(input, expected);
    }

    #[test]
    fn quotes() {
        let input = "echo 'one two \"three four\"' \"five six 'seven eight'\"";
        let expected = vec![
            "echo",
            "'one two \"three four\"'",
            "\"five six 'seven eight'\"",
        ];
        compare(input, expected);
    }
}
