/// Given an `input` string, this function will expand the variables that are discovered, using the closure provided
/// as the `action`. In the case of this program specifically, that action will be `self.get_var(&variable)`.
pub fn expand<F>(output: &mut String, input: &str, action: F)
    where F: Fn(&str) -> Option<String>
{
    for var_token in VariableIterator::new(&input.chars().collect::<Vec<char>>()) {
        match var_token {
            VariableToken::Normal(data) => {
                output.push_str(&data);
            },
            VariableToken::Variable(variable) => {
                if let Some(result) = action(&variable) {
                    output.push_str(&result);
                }
            }
        }
    }
}

/// A `VariableToken` is a token that signifies that the included text is either a `Variable` or simply `Normal` text.
#[derive(Debug, PartialEq)]
enum VariableToken {
    Variable(String),
    Normal(String),
}

/// A `VariableIterator` searches for variable patterns within a character array, returning `VariableToken`s.
struct VariableIterator<'a> {
    data:            &'a [char],
    buffer:          String,
    backslash:       bool,
    braced_variable: bool,
    variable_found:  bool,
    read:            usize,
}

impl<'a> VariableIterator<'a> {
    fn new(input: &'a [char]) -> VariableIterator<'a> {
        VariableIterator {
            data:            input,
            buffer:          String::with_capacity(128),
            backslash:       false,
            braced_variable: false,
            variable_found:  false,
            read:            0,
        }
    }
}

impl<'a> Iterator for VariableIterator<'a> {
    type Item = VariableToken;

    fn next(&mut self) -> Option<VariableToken> {
        for &character in self.data.iter().skip(self.read) {
            self.read += 1;
            if character == '\\' {
                if self.backslash { self.buffer.push(character); }
                self.backslash = !self.backslash;
            } else if self.backslash {
                match character {
                    '{' | '}' | '$' | ' ' | ':' | ',' | '@' | '#' => (),
                    _ => self.buffer.push('\\')
                }
                self.buffer.push(character);
                self.backslash = false;
            } else if self.braced_variable {
                if character == '}' {
                    let output = VariableToken::Variable(self.buffer.clone());
                    self.buffer.clear();
                    self.braced_variable = false;
                    return Some(output);
                } else {
                    self.buffer.push(character);
                }
            } else if self.variable_found {
                match character {
                    '{' => {
                        if self.read < 3 || (self.data[self.read-2] == '$' && self.data[self.read-3] != '\\') {
                            self.braced_variable = true;
                            self.variable_found  = false;
                        } else {
                            let output = VariableToken::Variable(self.buffer.clone());
                            self.buffer.clear();
                            self.buffer.push(character);
                            self.variable_found = false;
                            return Some(output);
                        }
                    },
                    '$' => {
                        let output = VariableToken::Variable(self.buffer.clone());
                        self.buffer.clear();
                        return Some(output);
                    },
                    ' ' | ':' | ',' | '@' | '#' | '}' => {
                        let output = VariableToken::Variable(self.buffer.clone());
                        self.buffer.clear();
                        self.buffer.push(character);
                        self.variable_found = false;
                        return Some(output);
                    },
                    _ => self.buffer.push(character),
                }
            } else {
                match character {
                    '$' if self.buffer.is_empty() => {
                        self.variable_found = true;
                    },
                    '$' => {
                        let output = VariableToken::Normal(self.buffer.clone());
                        self.buffer.clear();
                        self.variable_found = true;
                        return Some(output);
                    },
                    _ => self.buffer.push(character)
                }
            }
        }

        if self.buffer.is_empty() {
            None
        } else if self.variable_found {
            self.variable_found = false;
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
    let input = "${var1}${var2}  \\$\\\\$var1:$var2  $var1 $var2  abc${var1}def $var1,$var2";
    let input = input.chars().collect::<Vec<char>>();
    let tokens = VariableIterator::new(&input);
    let expected = vec![
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
    ];

    let mut correct = 0;
    for (actual, expected) in tokens.zip(expected.iter()) {
        assert_eq!(actual, *expected);
        correct += 1;
    }
    assert_eq!(expected.len(), correct);

    let input = "ABC${var1}DEF";
    let input = input.chars().collect::<Vec<char>>();
    let tokens = VariableIterator::new(&input);
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
