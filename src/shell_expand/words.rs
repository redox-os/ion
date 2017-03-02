use super::ExpandErr;

// Bit Twiddling Guide:
// var & FLAG == FLAG checks if FLAG is enabled
// var & FLAG != FLAG checks if FLAG is disabled
// var |= FLAG enables the FLAG
// var &= 255 ^ FLAG disables the FLAG
// var ^= FLAG swaps the state of FLAG

const WHITESPACE:   u8 = 1;
const SINGLE_QUOTE: u8 = 2;
const DOUBLE_QUOTE: u8 = 4;
const PREV_WAS_VAR: u8 = 8;
const BRACES:       u8 = 16;
const VARIABLES:    u8 = 32;
const TILDE:        u8 = 64;
const OPEN_BRACE:   u8 = 128;

#[derive(Debug, PartialEq)]
pub enum WordToken<'a> {
    Normal(&'a str),
    Tilde(&'a str),
    Brace(&'a str, bool),
    Variable(&'a str, bool)
}

pub struct WordIterator<'a> {
    data:  &'a str,
    read:  usize,
    flags: u8
}

impl<'a> WordIterator<'a> {
    pub fn new(data: &'a str) -> WordIterator<'a> {
        WordIterator { data: data, read: 0, flags: 0u8 }
    }
}

impl<'a> Iterator for WordIterator<'a> {
    type Item = Result<WordToken<'a>, ExpandErr>;

    fn next(&mut self) -> Option<Result<WordToken<'a>, ExpandErr>> {
        let mut start = self.read;
        let mut open_brace_id = 0;

        if self.flags & WHITESPACE == WHITESPACE {
            self.flags &= 255 ^ WHITESPACE;
            collect_whitespaces(&mut self.read, start, self.data).map(Ok)
        } else {
            let mut break_char = None;
            let mut backslash = false;
            self.flags &= 255 ^ (BRACES + TILDE + VARIABLES + PREV_WAS_VAR + OPEN_BRACE);
            for character in self.data.bytes().skip(self.read) {
                if backslash {
                    backslash = false;
                    if character == b'$' { self.flags |= VARIABLES; }
                } else if character == b'\\' {
                    backslash = true;
                    self.flags &= 255 ^ PREV_WAS_VAR;
                } else if character == b'\'' && self.flags & DOUBLE_QUOTE != DOUBLE_QUOTE {
                    if start != self.read {
                        let return_value = collect_at_single_quote(self.flags, start, self.read, self.data);
                        self.read += 1;
                        self.flags ^= SINGLE_QUOTE;
                        return Some(Ok(return_value));
                    }
                    start += 1;
                    self.flags ^= SINGLE_QUOTE
                } else if character == b'"' && self.flags & SINGLE_QUOTE != SINGLE_QUOTE {
                    if start != self.read {
                        let return_value = collect_at_double_quote(self.flags, start, self.read, self.data);
                        self.read += 1;
                        self.flags ^= DOUBLE_QUOTE;
                        return Some(Ok(return_value));
                    }
                    start += 1;
                    self.flags ^= DOUBLE_QUOTE;
                } else if character == b'{' && self.flags & (SINGLE_QUOTE + DOUBLE_QUOTE) == 0 {
                    if self.flags & PREV_WAS_VAR != PREV_WAS_VAR { self.flags |= BRACES; }
                    if self.flags & OPEN_BRACE == OPEN_BRACE { return Some(Err(ExpandErr::InnerBracesNotImplemented)); }
                    open_brace_id = self.read;
                    self.flags |= OPEN_BRACE;
                } else if character == b'}' && self.flags & (SINGLE_QUOTE + DOUBLE_QUOTE) == 0 {
                    if self.flags & OPEN_BRACE != OPEN_BRACE { return Some(Err(ExpandErr::UnmatchedBraces(self.read))); }
                    self.flags &= 255 ^ OPEN_BRACE;
                } else if self.flags & SINGLE_QUOTE != SINGLE_QUOTE && character == b'(' && self.flags & PREV_WAS_VAR == PREV_WAS_VAR {
                    break_char = Some(b')');
                    self.flags &= 255 ^ PREV_WAS_VAR;
                } else if self.flags & SINGLE_QUOTE != SINGLE_QUOTE && character == b'$' {
                    self.flags |= VARIABLES + PREV_WAS_VAR;
                } else if self.flags & (SINGLE_QUOTE + DOUBLE_QUOTE) == 0 && character == b'~' && start == self.read {
                    self.flags |= TILDE;
                    self.flags &= 255 ^ PREV_WAS_VAR;
                } else if character == b' ' && break_char.is_none() {
                    self.flags |= WHITESPACE;
                    return if self.flags & BRACES == BRACES {
                        Some(Ok(WordToken::Brace(&self.data[start..self.read], self.flags & VARIABLES == VARIABLES)))
                    } else if self.flags & VARIABLES == VARIABLES {
                        Some(Ok(WordToken::Variable(&self.data[start..self.read], self.flags & DOUBLE_QUOTE == DOUBLE_QUOTE)))
                    } else if self.flags & TILDE == TILDE {
                        Some(Ok(WordToken::Tilde(&self.data[start..self.read])))
                    } else {
                        Some(Ok(WordToken::Normal(&self.data[start..self.read])))
                    };
                } else if break_char == Some(character) {
                    break_char = None;
                } else {
                    self.flags &= 255 ^ PREV_WAS_VAR;
                }
                self.read += 1;
            }

            collect_at_end(self.flags, start, self.read, open_brace_id, self.data)
        }
    }
}

fn collect_whitespaces<'a>(read: &mut usize, start: usize, data: &'a str) -> Option<WordToken<'a>> {
    for character in data.bytes().skip(*read) {
        if character != b' ' {
            return Some(WordToken::Normal(&data[start..*read]));
        }
        *read += 1;
    }

    if start != *read { Some(WordToken::Normal(&data[start..*read])) } else { None }
}

fn collect_at_single_quote(flags: u8, start: usize, end: usize, data: &str) -> WordToken {
    if flags & SINGLE_QUOTE == SINGLE_QUOTE {
        WordToken::Normal(&data[start..end])
    } else if flags & BRACES == BRACES {
        WordToken::Brace(&data[start..end], flags & VARIABLES == VARIABLES)
    } else if flags & VARIABLES == VARIABLES {
        WordToken::Variable(&data[start..end], flags & DOUBLE_QUOTE == DOUBLE_QUOTE)
    } else if flags & TILDE == TILDE {
        WordToken::Tilde(&data[start..end])
    } else {
        WordToken::Normal(&data[start..end])
    }
}

fn collect_at_double_quote(flags: u8, start: usize, end: usize, data: &str) -> WordToken {
    if flags & SINGLE_QUOTE == SINGLE_QUOTE {
        if flags & VARIABLES == VARIABLES {
            WordToken::Variable(&data[start..end], flags & DOUBLE_QUOTE == DOUBLE_QUOTE)
        } else {
            WordToken::Normal(&data[start..end])
        }
    } else if flags & BRACES == BRACES {
        WordToken::Brace(&data[start..end], flags & VARIABLES == VARIABLES)
    } else if flags & VARIABLES == VARIABLES {
        WordToken::Variable(&data[start..end], flags & DOUBLE_QUOTE == DOUBLE_QUOTE)
    } else if flags & TILDE == TILDE {
        WordToken::Tilde(&data[start..end])
    } else {
        WordToken::Normal(&data[start..end])
    }
}

fn collect_at_end(flags: u8, start: usize, end: usize, open_brace_id: usize, data: &str)
    -> Option<Result<WordToken, ExpandErr>>
{
    if flags & OPEN_BRACE == OPEN_BRACE {
        Some(Err(ExpandErr::UnmatchedBraces(open_brace_id)))
    } else if start == end {
        None
    } else if flags & BRACES == BRACES {
        Some(Ok(WordToken::Brace(&data[start..end], flags & VARIABLES == VARIABLES)))
    } else if flags & VARIABLES == VARIABLES {
        Some(Ok(WordToken::Variable(&data[start..end], flags & DOUBLE_QUOTE == DOUBLE_QUOTE)))
    } else if flags & TILDE == TILDE {
        Some(Ok(WordToken::Tilde(&data[start..end])))
    } else {
        Some(Ok(WordToken::Normal(&data[start..end])))
    }
}

#[test]
fn test_malformed_brace_input() {
    assert_eq!(WordIterator::new("AB{CD").next(), Some(Err(ExpandErr::UnmatchedBraces(2))));
    assert_eq!(WordIterator::new("AB{{}").next(), Some(Err(ExpandErr::InnerBracesNotImplemented)));
    assert_eq!(WordIterator::new("AB}CD").next(), Some(Err(ExpandErr::UnmatchedBraces(2))));
}

#[test]
fn test_words() {
    let input = "echo $ABC ${ABC} one{$ABC,$ABC} ~ $(echo foo) \"$(seq 1 100)\"";
    let expected = vec![
        WordToken::Normal("echo"),
        WordToken::Normal(" "),
        WordToken::Variable("$ABC", false),
        WordToken::Normal(" "),
        WordToken::Variable("${ABC}", false),
        WordToken::Normal(" "),
        WordToken::Brace("one{$ABC,$ABC}", true),
        WordToken::Normal(" "),
        WordToken::Tilde("~"),
        WordToken::Normal(" "),
        WordToken::Variable("$(echo foo)", false),
        WordToken::Normal(" "),
        WordToken::Variable("$(seq 1 100)", true)
    ];

    let mut correct = 0;
    for (actual, expected) in WordIterator::new(input).zip(expected.iter()) {
        let actual = actual.expect(&format!("Expected {:?}", *expected));
        assert_eq!(actual, *expected, "{:?} != {:?}", actual, expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);
}
