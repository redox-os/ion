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
    buffer:          Vec<u8>,
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
            data:   input,
            buffer: Vec::with_capacity(128),
            flags:  0u8,
            read:   0,
        }
    }
}

impl<'a> Iterator for VariableIterator<'a> {
    type Item = VariableToken;

    fn next(&mut self) -> Option<VariableToken> {
        for character in self.data.bytes().skip(self.read) {
            self.read += 1;
            if character == b'\\' {
                if self.flags & BACK == BACK { self.buffer.push(character); }
                self.flags ^= BACK;
            } else if self.flags & BACK == BACK {
                match character {
                    b'{' | b'}' | b'(' | b')' | b'$' | b' ' | b':' | b',' | b'@' | b'#' => (),
                    _ => self.buffer.push(b'\\')
                }
                self.buffer.push(character);
                self.flags ^= BACK;
            } else if self.flags & BRACED_VAR == BRACED_VAR {
                if character == b'}' {
                    let output = VariableToken::Variable(convert_to_string(self.buffer.clone()));
                    self.buffer.clear();
                    self.flags &= 255 ^ BRACED_VAR;
                    return Some(output);
                } else {
                    self.buffer.push(character);
                }
            } else if self.flags & PARENS_VAR == PARENS_VAR {
                if character == b')' {
                    let output = VariableToken::Command(convert_to_string(self.buffer.clone()));
                    self.buffer.clear();
                    self.flags &= 255 ^ PARENS_VAR;
                    return Some(output);
                } else {
                    self.buffer.push(character);
                }
            } else if self.flags & VAR_FOUND == VAR_FOUND {
                match character {
                    b'{' => {
                        if self.read < 3 || (self.data.bytes().nth(self.read-2).unwrap() == b'$'
                            && self.data.bytes().nth(self.read-3).unwrap() != b'\\')
                        {
                            self.flags |= BRACED_VAR;
                            self.flags &= 255 ^ VAR_FOUND;
                        } else {
                            let output = VariableToken::Variable(convert_to_string(self.buffer.clone()));
                            self.buffer.clear();
                            self.buffer.push(character);
                            self.flags &= 255 ^ VAR_FOUND;
                            return Some(output);
                        }
                    },
                    b'(' => {
                        if self.read < 3 || (self.data.bytes().nth(self.read-2).unwrap() == b'$'
                            && self.data.bytes().nth(self.read-3).unwrap() != b'\\')
                        {
                            self.flags |= PARENS_VAR;
                            self.flags &= 255 ^ VAR_FOUND;
                        } else {
                            let output = VariableToken::Variable(convert_to_string(self.buffer.clone()));
                            self.buffer.clear();
                            self.buffer.push(character);
                            self.flags &= 255 ^ VAR_FOUND;
                            return Some(output);
                        }
                    },
                    b'$' => {
                        let output = VariableToken::Variable(convert_to_string(self.buffer.clone()));
                        self.buffer.clear();
                        return Some(output);
                    },
                    b' ' | b':' | b',' | b'@' | b'#' | b'}' | b')' => {
                        let output = VariableToken::Variable(convert_to_string(self.buffer.clone()));
                        self.buffer.clear();
                        self.buffer.push(character);
                        self.flags &= 255 ^ VAR_FOUND;
                        return Some(output);
                    },
                    _ => self.buffer.push(character),
                }
            } else {
                match character {
                    b'$' if self.buffer.is_empty() => {
                        self.flags |= VAR_FOUND;
                    },
                    b'$' => {
                        let output = VariableToken::Normal(convert_to_string(self.buffer.clone()));
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
            let output = VariableToken::Variable(convert_to_string(self.buffer.clone()));
            self.buffer.clear();
            Some(output)
        } else {
            let output = VariableToken::Normal(convert_to_string(self.buffer.clone()));
            self.buffer.clear();
            Some(output)
        }
    }
}

#[inline]
fn convert_to_string(data: Vec<u8>) -> String {
    unsafe { String::from_utf8_unchecked(data) }
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
