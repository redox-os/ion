/// Given an `input` string, this function will expand the variables that are discovered, using the closure provided
/// as the `action`. In the case of this program specifically, that action will be `self.get_var(&variable)`.
pub fn expand<V, C>(output: &mut String, input: &str, expand_variable: V, expand_command: C)
    where V: Fn(&str) -> Option<String>,
          C: Fn(&str) -> Option<String>
{
    for var_token in VariableIterator::new(input) {
        match var_token {
            VariableToken::Normal(data) => {
                output.push_str(&data);
            },
            VariableToken::Variable(variable) => {
                if let Some(result) = expand_variable(&variable) {
                    output.push_str(&result);
                }
            },
            VariableToken::Command(command) => {
                if let Some(result) = expand_command(&command) {
                    output.push_str(&result);
                }
            }
        }
    }
}

/// A `VariableToken` is a token that signifies that the included text is either a `Variable` or simply `Normal` text.
#[derive(Debug, PartialEq)]
enum VariableToken {
    Normal(String),
    Variable(String),
    Command(String),
}

/// A `VariableIterator` searches for variable patterns within a character array, returning `VariableToken`s.
struct VariableIterator<'a> {
    data:            &'a str,
    buffer:          String,
    flags:           u8,
    read:            usize,
}

/// Bit flags used by `VariableIterator`'s flags field.
const BACK:       u8 = 1;
const BRACED_VAR: u8 = 2;
const PARENS_VAR: u8 = 4;
const VAR_FOUND:  u8 = 8;

impl<'a> VariableIterator<'a> {
    fn new(input: &'a str) -> VariableIterator<'a> {
        VariableIterator {
            data:            input,
            buffer:          String::with_capacity(128),
            flags:           0u8,
            read:            0,
        }
    }
}

impl<'a> Iterator for VariableIterator<'a> {
    type Item = VariableToken;

    fn next(&mut self) -> Option<VariableToken> {
        for character in self.data.chars().skip(self.read) {
            self.read += 1;
            if character == '\\' {
                if self.flags & BACK == BACK { self.buffer.push(character); }
                self.flags ^= BACK;
            } else if self.flags & BACK == BACK {
                match character {
                    '{' | '}' | '(' | ')' | '$' | ' ' | ':' | ',' | '@' | '#' => (),
                    _ => self.buffer.push('\\')
                }
                self.buffer.push(character);
                self.flags ^= BACK;
            } else if self.flags & BRACED_VAR == BRACED_VAR {
                if character == '}' {
                    let output = VariableToken::Variable(self.buffer.clone());
                    self.buffer.clear();
                    self.flags &= 255 ^ BRACED_VAR;
                    return Some(output);
                } else {
                    self.buffer.push(character);
                }
            } else if self.flags & PARENS_VAR == PARENS_VAR {
                if character == ')' {
                    let output = VariableToken::Command(self.buffer.clone());
                    self.buffer.clear();
                    self.flags &= 255 ^ PARENS_VAR;
                    return Some(output);
                } else {
                    self.buffer.push(character);
                }
            } else if self.flags & VAR_FOUND == VAR_FOUND {
                match character {
                    '{' => {
                        if self.read < 3 || (self.data.chars().nth(self.read-2).unwrap() == '$'
                            && self.data.chars().nth(self.read-3).unwrap() != '\\')
                        {
                            self.flags |= BRACED_VAR;
                            self.flags &= 255 ^ VAR_FOUND;
                        } else {
                            let output = VariableToken::Variable(self.buffer.clone());
                            self.buffer.clear();
                            self.buffer.push(character);
                            self.flags &= 255 ^ VAR_FOUND;
                            return Some(output);
                        }
                    },
                    '(' => {
                        if self.read < 3 || (self.data.chars().nth(self.read-2).unwrap() == '$'
                            && self.data.chars().nth(self.read-3).unwrap() != '\\')
                        {
                            self.flags |= PARENS_VAR;
                            self.flags &= 255 ^ VAR_FOUND;
                        } else {
                            let output = VariableToken::Variable(self.buffer.clone());
                            self.buffer.clear();
                            self.buffer.push(character);
                            self.flags &= 255 ^ VAR_FOUND;
                            return Some(output);
                        }
                    },
                    '$' => {
                        let output = VariableToken::Variable(self.buffer.clone());
                        self.buffer.clear();
                        return Some(output);
                    },
                    ' ' | ':' | ',' | '@' | '#' | '}' | ')' => {
                        let output = VariableToken::Variable(self.buffer.clone());
                        self.buffer.clear();
                        self.buffer.push(character);
                        self.flags &= 255 ^ VAR_FOUND;
                        return Some(output);
                    },
                    _ => self.buffer.push(character),
                }
            } else {
                match character {
                    '$' if self.buffer.is_empty() => {
                        self.flags |= VAR_FOUND;
                    },
                    '$' => {
                        let output = VariableToken::Normal(self.buffer.clone());
                        self.buffer.clear();
                        self.flags |= VAR_FOUND;
                        return Some(output);
                    },
                    _ => self.buffer.push(character)
                }
            }
        }

        if self.buffer.is_empty() {
            None
        } else if self.flags & VAR_FOUND == VAR_FOUND {
            self.flags &= 255 ^ VAR_FOUND;
            let output = VariableToken::Variable(self.buffer.clone());
            self.buffer.clear();
            Some(output)
        } else {
            let output = VariableToken::Normal(self.buffer.clone());
            self.buffer.clear();
            Some(output)
        }
    }
}

#[test]
fn test_variables() {
    let input = "$(echo bar) ${var1}${var2}  \\$\\\\$var1:$var2  $var1 $var2  abc${var1}def $var1,$var2 $(echo foo)";
    let tokens = VariableIterator::new(input);
    let expected = vec![
        VariableToken::Command("echo bar".to_string()),
        VariableToken::Normal(" ".to_string()),
        VariableToken::Variable("var1".to_string()),
        VariableToken::Variable("var2".to_string()),
        VariableToken::Normal("  $\\".to_string()),
        VariableToken::Variable("var1".to_string()),
        VariableToken::Normal(":".to_string()),
        VariableToken::Variable("var2".to_string()),
        VariableToken::Normal("  ".to_string()),
        VariableToken::Variable("var1".to_string()),
        VariableToken::Normal(" ".to_string()),
        VariableToken::Variable("var2".to_string()),
        VariableToken::Normal("  abc".to_string()),
        VariableToken::Variable("var1".to_string()),
        VariableToken::Normal("def ".to_string()),
        VariableToken::Variable("var1".to_string()),
        VariableToken::Normal(",".to_string()),
        VariableToken::Variable("var2".to_string()),
        VariableToken::Normal(" ".to_string()),
        VariableToken::Command("echo foo".to_string()),
    ];

    let mut correct = 0;
    for (actual, expected) in tokens.zip(expected.iter()) {
        assert_eq!(actual, *expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);

    let input = "ABC${var1}DEF";
    let tokens = VariableIterator::new(input);
    let expected = vec![
        VariableToken::Normal("ABC".to_string()),
        VariableToken::Variable("var1".to_string()),
        VariableToken::Normal("DEF".to_string()),
    ];

    let mut correct = 0;
    for (actual, expected) in tokens.zip(expected.iter()) {
        assert_eq!(actual, *expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);
}
