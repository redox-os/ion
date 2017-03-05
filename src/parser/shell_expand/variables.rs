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
                // Expand variables within the command before executing the command within a subshell.
                let mut expanded = String::with_capacity(command.len());
                for token in CommandExpander::new(&command) {
                    match token {
                        CommandToken::Normal(string) => expanded.push_str(string),
                        CommandToken::Variable(var) => {
                            if let Some(result) = expand_variable(var) {
                                expanded.push_str(&result);
                            }
                        }
                    }
                }

                if let Some(result) = expand_command(&expanded) {
                    output.push_str(&result);
                }
            }
        }
    }
}

/// Bit flags used by `VariableIterator`'s and `CommandExpander`'s flags fields.
const BACK:       u8 = 1;
const BRACED_VAR: u8 = 2;
const PARENS_VAR: u8 = 4;
const VAR_FOUND:  u8 = 8;

/// Commands need to have variables expanded before they are submitted. This custom `Iterator` structure is
/// responsible for slicing the variables from within commands so that they can be expanded beforehand.
struct CommandExpander<'a> {
    data:  &'a str,
    read:  usize,
    flags: u8,
}

impl<'a> CommandExpander<'a> {
    fn new(data: &'a str) -> CommandExpander<'a> {
        CommandExpander { data: data, read: 0, flags: 0 }
    }
}

enum CommandToken<'a> {
    Normal(&'a str),
    Variable(&'a str),
}

impl<'a> Iterator for CommandExpander<'a> {
    type Item = CommandToken<'a>;

    fn next(&mut self) -> Option<CommandToken<'a>> {
        let start = self.read;
        let mut iterator = self.data.bytes().skip(self.read);

        while let Some(character) = iterator.next()  {
            self.read += 1;
            match character {
                b'\\' => { self.flags ^= BACK; continue },
                b'$' if self.flags & (VAR_FOUND + BACK) == 0 => {
                    if let Some(character) = self.data.bytes().nth(self.read) {
                        if character == b'(' { continue }
                    }

                    self.flags |= VAR_FOUND;
                    if start != self.read {
                        return Some(CommandToken::Normal(&self.data[start..self.read-1]));
                    }
                },
                _ if self.flags & VAR_FOUND != 0 => {
                    self.flags ^= VAR_FOUND;
                    if character == b'{' {
                        // Slice the braced variable from the command string.
                        while let Some(character) = iterator.next() {
                            self.read += 1;
                            match character {
                                b'\\' => { self.flags ^= BACK; self.read += 1; continue },
                                b'}' if self.flags & BACK == 0 => {
                                    return Some(CommandToken::Variable(&self.data[start+1..self.read-1]))
                                },
                                _ => ()
                            }
                            self.flags &= 255 ^ BACK;
                        }
                    } else {
                        // Slice the non-braced variable from the command string.
                        for character in iterator {
                            match character {
                                b'$' => {
                                    self.flags |= VAR_FOUND;
                                    self.read += 1;
                                    return Some(CommandToken::Variable(&self.data[start..self.read-1]))
                                }
                                b'{' | b'}' | b'(' | b')' | b' ' | b':' | b',' | b'@' | b'#' =>
                                    return Some(CommandToken::Variable(&self.data[start..self.read])),
                                _ => ()
                            }
                            self.read += 1;
                        }

                        return Some(CommandToken::Variable(&self.data[start..self.read]));
                    }
                },
                _ => ()
            }
            self.flags &= 255 ^ BACK;
        }

        if start == self.read { None } else { Some(CommandToken::Normal(&self.data[start..])) }
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
    data:   &'a str,
    buffer: Vec<u8>,
    flags:  u8,
    read:   usize,
}

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
        let mut levels = 0;
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
                if character == b'$' {
                    self.buffer.push(character);
                    self.flags |= VAR_FOUND;
                    continue
                } else if character == b')' {
                    levels -= 1;
                    if levels == 0 {
                        let output = VariableToken::Command(convert_to_string(self.buffer.clone()));
                        self.buffer.clear();
                        self.flags &= 255 ^ PARENS_VAR;
                        return Some(output);
                    }
                } else if character == b'(' && self.flags & VAR_FOUND != 0 {
                    levels += 1;
                }
                self.flags &= 255 ^ VAR_FOUND;
                self.buffer.push(character);
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
                            levels += 1;
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
