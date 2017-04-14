const DOUBLE: u8 = 1;
const SINGLE: u8 = 2;
const BACK:   u8 = 4;
const COMM_1: u8 = 8;
const COMM_2: u8 = 16;
const VARIAB: u8 = 32;
const ARRAY:  u8 = 64;
const METHOD: u8 = 128;

/// An efficient `Iterator` structure for splitting arguments
pub struct ArgumentSplitter<'a> {
    data:         &'a str,
    read:         usize,
    flags:        u8,
}

impl<'a> ArgumentSplitter<'a> {
    pub fn new(data: &'a str) -> ArgumentSplitter<'a> {
        ArgumentSplitter {
            data:         data,
            read:         0,
            flags:        0,
        }
    }
}

impl<'a> Iterator for ArgumentSplitter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        while let Some(&b' ') = self.data.as_bytes().get(self.read) {
            self.read += 1;
        }
        let start = self.read;

        let (mut level, mut array_level, mut array_process_level) = (0, 0, 0);
        for character in self.data.bytes().skip(self.read) {
            match character {
                _ if self.flags & BACK != 0 => self.flags ^= BACK,
                b'\\'                       => self.flags ^= BACK,
                b'@' if self.flags & SINGLE == 0 => {
                    self.flags &= 255 ^ COMM_1;
                    self.flags |= COMM_2 + ARRAY;
                    self.read += 1;
                    continue
                },
                b'$' if self.flags & SINGLE == 0 => {
                    self.flags &= 255 ^ COMM_2;
                    self.flags |= COMM_1 + VARIAB;
                    self.read += 1;
                    continue
                },
                b'['  if self.flags & SINGLE == 0 && self.flags & COMM_2 != 0 => array_process_level += 1,
                b'['  if self.flags & SINGLE == 0 => array_level += 1,
                b']'  if self.flags & SINGLE == 0 && array_level != 0 => array_level -= 1,
                b']'  if self.flags & SINGLE == 0 => array_process_level -= 1,
                b'('  if self.flags & SINGLE == 0 && self.flags & COMM_1 != 0 => level += 1,
                b'('  if self.flags & SINGLE == 0 && self.flags & (VARIAB + ARRAY) != 0 => {
                    self.flags |= METHOD;
                    self.flags &= 255 ^ (VARIAB + ARRAY);
                },
                b')'  if self.flags & SINGLE == 0 && self.flags & METHOD != 0 => {
                    self.flags &= 255 ^ METHOD;
                },
                b')'  if self.flags & SINGLE == 0 => level -= 1,
                b'"'  if self.flags & SINGLE == 0 => self.flags ^= DOUBLE,
                b'\'' if self.flags & DOUBLE == 0 => self.flags ^= SINGLE,
                b' '  if self.flags & (SINGLE + DOUBLE + METHOD) == 0
                    && level == 0 && array_level == 0 && array_process_level == 0 => break,
                _ => ()
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
