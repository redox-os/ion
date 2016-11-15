use super::ExpandErr;

#[derive(Debug, PartialEq)]
pub enum WordToken<'a> {
    Normal(&'a str),
    Tilde(&'a str),
    Brace(&'a str, bool),
    Variable(&'a str)
}

pub struct WordIterator<'a> {
    data: &'a str,
    read: usize,
    whitespace: bool,
}

impl<'a> WordIterator<'a> {
    pub fn new(data: &'a str) -> WordIterator<'a> {
        WordIterator { data: data, read: 0, whitespace: false }
    }
}

impl<'a> Iterator for WordIterator<'a> {
    type Item = Result<WordToken<'a>, ExpandErr>;

    fn next(&mut self) -> Option<Result<WordToken<'a>, ExpandErr>> {
        let start = self.read;
        let mut open_brace_id = 0;

        if self.whitespace {
            self.whitespace = false;
            for character in self.data.chars().skip(self.read) {
                if character != ' ' {
                    return Some(Ok(WordToken::Normal(&self.data[start..self.read])));
                }
                self.read += 1;
            }

            if start != self.read {
                Some(Ok(WordToken::Normal(&self.data[start..self.read])))
            } else {
                None
            }
        } else {
            let (mut contains_braces, mut contains_variables, mut contains_tilde,
                mut backslash, mut previous_char_was_dollar, mut open_brace) =
                    (false, false, false, false, false, false);
            for character in self.data.chars().skip(self.read) {
                if backslash {
                    if character == '$' { contains_variables = true; }
                    backslash = false;
                } else if character == '\\' {
                    backslash = true;
                    previous_char_was_dollar = false;
                } else if character == '{' {
                    if !previous_char_was_dollar { contains_braces = true; }
                    if open_brace { return Some(Err(ExpandErr::InnerBracesNotImplemented)); }
                    open_brace_id = self.read;
                    open_brace = true;
                } else if character == '}' {
                    if !open_brace { return Some(Err(ExpandErr::UnmatchedBraces(self.read))); }
                    open_brace = false;
                } else if character == '$' {
                    contains_variables = true;
                    previous_char_was_dollar = true;
                } else if character == '~' && start == self.read {
                    contains_tilde = true;
                } else if character == ' ' {
                    self.whitespace = true;
                    return if contains_braces {
                        Some(Ok(WordToken::Brace(&self.data[start..self.read], contains_variables)))
                    } else if contains_variables {
                        Some(Ok(WordToken::Variable(&self.data[start..self.read])))
                    } else if contains_tilde {
                        Some(Ok(WordToken::Tilde(&self.data[start..self.read])))
                    } else {
                        Some(Ok(WordToken::Normal(&self.data[start..self.read])))
                    };
                } else {
                    previous_char_was_dollar = false;
                }
                self.read += 1;
            }

            if open_brace {
                Some(Err(ExpandErr::UnmatchedBraces(open_brace_id)))
            } else if start == self.read {
                None
            } else if contains_braces {
                Some(Ok(WordToken::Brace(&self.data[start..self.read], contains_variables)))
            } else if contains_variables {
                Some(Ok(WordToken::Variable(&self.data[start..self.read])))
            } else if contains_tilde {
                Some(Ok(WordToken::Tilde(&self.data[start..self.read])))
            } else {
                Some(Ok(WordToken::Normal(&self.data[start..self.read])))
            }
        }
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
    let input = "echo $ABC ${ABC} one{$ABC,$ABC} ~";
    let expected = vec![
        WordToken::Normal("echo"),
        WordToken::Normal(" "),
        WordToken::Variable("$ABC"),
        WordToken::Normal(" "),
        WordToken::Variable("${ABC}"),
        WordToken::Normal(" "),
        WordToken::Brace("one{$ABC,$ABC}", true),
        WordToken::Normal(" "),
        WordToken::Tilde("~")
    ];

    for (actual, expected) in WordIterator::new(input).zip(expected.iter()) {
        let actual = actual.expect(&format!("Expected {:?}", *expected));
        assert_eq!(actual, *expected, "{:?} != {:?}", actual, expected);
    }
}
