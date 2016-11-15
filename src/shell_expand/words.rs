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
    type Item = WordToken<'a>;

    fn next(&mut self) -> Option<WordToken<'a>> {
        let start = self.read;

        if self.whitespace {
            self.whitespace = false;
            for character in self.data.chars().skip(self.read) {
                if character != ' ' {
                    return Some(WordToken::Normal(&self.data[start..self.read]));
                }
                self.read += 1;
            }

            if start != self.read {
                Some(WordToken::Normal(&self.data[start..self.read]))
            } else {
                None
            }
        } else {
            let mut break_char = None;
            let (mut contains_braces, mut contains_variables, mut contains_tilde,
                mut backslash, mut previous_char_was_dollar) = (false, false, false, false, false);
            for character in self.data.chars().skip(self.read) {
                if backslash {
                    if character == '$' { contains_variables = true; }
                    backslash = false;
                } else if character == '\\' {
                    backslash = true;
                    previous_char_was_dollar = false;
                } else if character == '{' && !previous_char_was_dollar {
                    contains_braces = true;
                } else if character == '(' && previous_char_was_dollar {
                    break_char = Some(')');
                    previous_char_was_dollar = false;
                } else if character == '$' {
                    contains_variables = true;
                    previous_char_was_dollar = true;
                } else if character == '~' && start == self.read {
                    contains_tilde = true;
                } else if character == ' ' && break_char.is_none() {
                    self.whitespace = true;
                    return if contains_braces {
                        Some(WordToken::Brace(&self.data[start..self.read], contains_variables))
                    } else if contains_variables {
                        Some(WordToken::Variable(&self.data[start..self.read]))
                    } else if contains_tilde {
                        Some(WordToken::Tilde(&self.data[start..self.read]))
                    } else {
                        Some(WordToken::Normal(&self.data[start..self.read]))
                    };
                } else if break_char == Some(character) {
                    break_char = None;
                } else {
                    previous_char_was_dollar = false;
                }
                self.read += 1;
            }

            if start == self.read { return None; }
            if contains_braces {
                Some(WordToken::Brace(&self.data[start..self.read], contains_variables))
            } else if contains_variables {
                Some(WordToken::Variable(&self.data[start..self.read]))
            } else if contains_tilde {
                Some(WordToken::Tilde(&self.data[start..self.read]))
            } else {
                Some(WordToken::Normal(&self.data[start..self.read]))
            }
        }
    }
}

#[test]
fn test_words() {
    let input = "echo $ABC ${ABC} one{$ABC,$ABC} ~ $(echo foo)";
    let expected = vec![
        WordToken::Normal("echo"),
        WordToken::Normal(" "),
        WordToken::Variable("$ABC"),
        WordToken::Normal(" "),
        WordToken::Variable("${ABC}"),
        WordToken::Normal(" "),
        WordToken::Brace("one{$ABC,$ABC}", true),
        WordToken::Normal(" "),
        WordToken::Tilde("~"),
        WordToken::Normal(" "),
        WordToken::Variable("$(echo foo)")
    ];

    let mut correct = 0;
    for (actual, expected) in WordIterator::new(input).zip(expected.iter()) {
        assert_eq!(actual, *expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);
}
