use super::ExpandErr;

#[derive(Debug, PartialEq)]
pub enum WordToken<'a> {
    Normal(&'a str),
    Tilde(&'a str),
    Brace(&'a str, bool),
    Variable(&'a str, bool)
}

pub struct WordIterator<'a> {
    data:         &'a str,
    read:         usize,
    whitespace:   bool,
    single_quote: bool,
    double_quote: bool,
}

impl<'a> WordIterator<'a> {
    pub fn new(data: &'a str) -> WordIterator<'a> {
        WordIterator { data: data, read: 0, whitespace: false, single_quote: false, double_quote: false }
    }
}

fn collect_whitespaces<'a>(read: &mut usize, start: usize, data: &'a str) -> Option<WordToken<'a>> {
    for character in data.chars().skip(*read) {
        if character != ' ' {
            return Some(WordToken::Normal(&data[start..*read]));
        }
        *read += 1;
    }

    if start != *read { Some(WordToken::Normal(&data[start..*read])) } else { None }
}

impl<'a> Iterator for WordIterator<'a> {
    type Item = Result<WordToken<'a>, ExpandErr>;

    fn next(&mut self) -> Option<Result<WordToken<'a>, ExpandErr>> {
        let mut start = self.read;
        let mut open_brace_id = 0;

        if self.whitespace {
            self.whitespace = false;
            collect_whitespaces(&mut self.read, start, self.data).map(Ok)
        } else {
            let mut break_char = None;
            let (mut contains_braces, mut contains_variables, mut contains_tilde,
                mut backslash, mut previous_char_was_dollar, mut open_brace) =
                    (false, false, false, false, false, false);
            for character in self.data.chars().skip(self.read) {
                if backslash {
                    backslash = false;
                    if character == '$' { contains_variables = true; }
                } else if character == '\\' {
                    backslash = true;
                    previous_char_was_dollar = false;
                } else if character == '\'' && !self.double_quote {
                    if start != self.read {
                        let return_value = if self.single_quote {
                            Ok(WordToken::Normal(&self.data[start..self.read]))
                        } else if contains_braces {
                            Ok(WordToken::Brace(&self.data[start..self.read], contains_variables))
                        } else if contains_variables {
                            Ok(WordToken::Variable(&self.data[start..self.read], self.double_quote))
                        } else if contains_tilde {
                            Ok(WordToken::Tilde(&self.data[start..self.read]))
                        } else {
                            Ok(WordToken::Normal(&self.data[start..self.read]))
                        };
                        self.read += 1;
                        self.single_quote = !self.single_quote;
                        return Some(return_value);
                    }
                    start += 1;
                    self.single_quote = !self.single_quote;
                } else if character == '"' && !self.single_quote {
                    if start != self.read {
                        let return_value = if self.single_quote {
                            if contains_variables {
                                Ok(WordToken::Variable(&self.data[start..self.read], self.double_quote))
                            } else {
                                Ok(WordToken::Normal(&self.data[start..self.read]))
                            }
                        } else if contains_braces {
                            Ok(WordToken::Brace(&self.data[start..self.read], contains_variables))
                        } else if contains_variables {
                            Ok(WordToken::Variable(&self.data[start..self.read], self.double_quote))
                        } else if contains_tilde {
                            Ok(WordToken::Tilde(&self.data[start..self.read]))
                        } else {
                            Ok(WordToken::Normal(&self.data[start..self.read]))
                        };
                        self.read += 1;
                        self.double_quote = !self.double_quote;
                        return Some(return_value);
                    }
                    start += 1;
                    self.double_quote = !self.double_quote;
                } else if character == '{' && !self.single_quote && !self.double_quote {
                    if !previous_char_was_dollar { contains_braces = true; }
                    if open_brace { return Some(Err(ExpandErr::InnerBracesNotImplemented)); }
                    open_brace_id = self.read;
                    open_brace = true;
                } else if character == '}' && !self.single_quote && !self.double_quote {
                    if !open_brace { return Some(Err(ExpandErr::UnmatchedBraces(self.read))); }
                    open_brace = false;
                } else if !self.single_quote && character == '(' && previous_char_was_dollar {
                    break_char = Some(')');
                    previous_char_was_dollar = false;
                } else if !self.single_quote && character == '$' {
                    contains_variables = true;
                    previous_char_was_dollar = true;
                } else if !self.single_quote && !self.double_quote && character == '~' && start == self.read {
                    contains_tilde = true;
                    previous_char_was_dollar = false;
                } else if character == ' ' && break_char.is_none() {
                    self.whitespace = true;
                    return if contains_braces {
                        Some(Ok(WordToken::Brace(&self.data[start..self.read], contains_variables)))
                    } else if contains_variables {
                        Some(Ok(WordToken::Variable(&self.data[start..self.read], self.double_quote)))
                    } else if contains_tilde {
                        Some(Ok(WordToken::Tilde(&self.data[start..self.read])))
                    } else {
                        Some(Ok(WordToken::Normal(&self.data[start..self.read])))
                    };
                } else if break_char == Some(character) {
                    break_char = None;
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
                Some(Ok(WordToken::Variable(&self.data[start..self.read], self.double_quote)))
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
