pub enum WordToken {
    Normal(String),
    Tilde(String),
    Brace(String, bool),
    Variable(String)
}

pub struct WordIterator<'a> {
    data: &'a str,
    read: usize,
    whitespace: bool,
    complete:   bool,
}

impl<'a> WordIterator<'a> {
    pub fn new(data: &'a str) -> WordIterator<'a> {
        WordIterator { data: data, read: 0, whitespace: false, complete: false }
    }
}

impl<'a> Iterator for WordIterator<'a> {
    type Item = WordToken;

    fn next(&mut self) -> Option<WordToken> {
        if self.complete { return None; }
        let mut output = "".to_owned();

        if self.whitespace {
            self.whitespace = false;
            for character in self.data.chars().skip(self.read) {
                if character != ' ' {
                    return Some(WordToken::Normal(output));
                }
                self.read += 1;
                output.push(character);
            }
            if !output.is_empty() {
                Some(WordToken::Normal(output))
            } else {
                None
            }
        } else {
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
                } else if character == '$' {
                    contains_variables = true;
                    previous_char_was_dollar = true;
                } else if character == '~' && output.is_empty() {
                    contains_tilde = true;
                } else if character == ' ' {
                    self.whitespace = true;
                    return if contains_braces {
                        Some(WordToken::Brace(output, contains_variables))
                    } else if contains_variables {
                        Some(WordToken::Variable(output))
                    } else if contains_tilde {
                        Some(WordToken::Tilde(output))
                    } else {
                        Some(WordToken::Normal(output))
                    };
                } else {
                    previous_char_was_dollar = false;
                }
                self.read += 1;
                output.push(character);
            }

            if output.is_empty() { return None; }
            self.complete = true;
            if contains_braces {
                Some(WordToken::Brace(output, contains_variables))
            } else if contains_variables {
                Some(WordToken::Variable(output))
            } else if contains_tilde {
                Some(WordToken::Tilde(output))
            } else {
                Some(WordToken::Normal(output))
            }
        }
    }
}
