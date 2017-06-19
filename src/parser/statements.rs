// TODO:
// - Rewrite this in the same style as shell_expand::words.
// - Validate syntax in methods

use std::u16;
use std::io::{self, Write};
use shell::flow_control::Statement;
use super::peg::parse;

const SQUOTE: u16 = 1;
const DQUOTE: u16 = 2;
const BACKSL: u16 = 4;
const COMM_1: u16 = 8;
const COMM_2: u16 = 16;
const VBRACE: u16 = 32;
const ARRAY:  u16 = 64;
const VARIAB: u16 = 128;
const METHOD: u16 = 256;


#[derive(Debug, PartialEq)]
pub enum StatementError<'a> {
    IllegalCommandName(&'a str),
    InvalidCharacter(char, usize),
    UnterminatedSubshell,
    UnterminatedBracedVar,
    UnterminatedBrace,
    UnterminatedMethod,
    ExpectedCommandButFound(&'static str)
}

pub fn check_statement<'a>(statement: Result<&str, StatementError<'a>>) -> Statement {
    match statement {
        Ok(statement) => parse(statement),
        Err(err) => {
            let stderr = io::stderr();
            match err {
                StatementError::IllegalCommandName(command) => {
                    let _ = writeln!(stderr.lock(), "ion: illegal command name: {}", command);
                }
                StatementError::InvalidCharacter(character, position) => {
                    let _ = writeln!(stderr.lock(),
                        "ion: syntax error: '{}' at position {} is out of place",
                        character, position);
                },
                StatementError::UnterminatedSubshell => {
                    let _ = writeln!(stderr.lock(), "ion: syntax error: unterminated subshell");
                },
                StatementError::UnterminatedBrace => {
                    let _ = writeln!(stderr.lock(), "ion: syntax error: unterminated brace");
                },
                StatementError::UnterminatedBracedVar => {
                    let _ = writeln!(stderr.lock(), "ion: syntax error: unterminated braced var");
                },
                StatementError::UnterminatedMethod => {
                    let _ = writeln!(stderr.lock(), "ion: syntax error: unterminated method");
                }
                StatementError::ExpectedCommandButFound(element) => {
                    let _ = writeln!(stderr.lock(), "ion: expected command, but found {}", element);
                }
            }
            Statement::Error(-1)
        }
    }
}

pub struct StatementSplitter<'a> {
    data:  &'a str,
    read:  usize,
    flags: u16,
    array_level: u8,
    array_process_level: u8,
    process_level: u8,
    brace_level: u8,
}

impl<'a> StatementSplitter<'a> {
    pub fn new(data: &'a str) -> StatementSplitter<'a> {
        StatementSplitter {
            data: data,
            read: 0,
            flags: 0,
            array_level: 0,
            array_process_level: 0,
            process_level: 0,
            brace_level: 0
        }
    }
}

impl<'a> Iterator for StatementSplitter<'a> {
    type Item = Result<&'a str, StatementError<'a>>;
    fn next(&mut self) -> Option<Result<&'a str, StatementError<'a>>> {
        let start = self.read;
        let mut first_arg_found = false;
        let mut else_found = false;
        let mut else_pos = 0;
        let mut error = None;
        for character in self.data.bytes().skip(self.read) {
            self.read += 1;
            match character {
                0...47 | 58...64 | 91...94 | 96 | 123...124 | 126...127 if self.flags & VBRACE != 0 => {
                    // If we are just ending the braced section continue as normal
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(character as char, self.read))
                    }
                },
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                b'@'  if self.flags & SQUOTE == 0 => {
                    self.flags &= u16::MAX ^ COMM_1;
                    self.flags |= COMM_2 + ARRAY;
                    continue
                }
                b'$'  if self.flags & SQUOTE == 0 => {
                    self.flags &= u16::MAX ^ COMM_2;
                    self.flags |= COMM_1 + VARIAB;
                    continue
                },
                b'{'  if self.flags & (COMM_1 + COMM_2) != 0 => self.flags |= VBRACE,
                b'{'  if self.flags & (SQUOTE + DQUOTE) == 0 => self.brace_level += 1,
                b'}'  if self.flags & VBRACE != 0 => self.flags ^= VBRACE,
                b'}'  if self.flags & (SQUOTE + DQUOTE) == 0 => {
                    if self.brace_level == 0 {
                        if error.is_none() {
                            error = Some(StatementError::InvalidCharacter(character as char, self.read))
                        }
                    } else {
                        self.brace_level -= 1;
                    }
                },
                b'('  if self.flags & (COMM_1 + VARIAB + ARRAY) == 0 => {
                    if error.is_none() && self.flags & (SQUOTE + DQUOTE) == 0 {
                        error = Some(StatementError::InvalidCharacter(character as char, self.read))
                    }
                },
                b'(' if self.flags & COMM_1 != 0 => {
                    self.process_level += 1;
                    self.flags &= u16::MAX ^ (VARIAB + ARRAY);
                },
                b'(' if self.flags & (VARIAB + ARRAY) != 0 => {
                    self.flags &= u16::MAX ^ (VARIAB + ARRAY);
                    self.flags |= METHOD;
                },
                b'[' if self.flags & COMM_2 != 0 => {
                    self.array_process_level += 1;
                },
                b'[' if self.flags & SQUOTE == 0 => self.array_level += 1,
                b']' if self.array_process_level == 0 && self.array_level == 0 && self.flags & SQUOTE == 0 => {
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(character as char, self.read))
                    }
                },
                b']' if self.flags & SQUOTE == 0 && self.array_level != 0 => self.array_level -= 1,
                b']' if self.flags & SQUOTE == 0 => self.array_process_level -= 1,
                b')' if self.flags & SQUOTE == 0 && self.flags & METHOD != 0 => {
                    self.flags ^= METHOD;
                },
                b')' if self.process_level == 0 && self.array_level == 0 && self.flags & SQUOTE == 0 => {
                    if error.is_none() && self.flags & (SQUOTE + DQUOTE) == 0 {
                        error = Some(StatementError::InvalidCharacter(character as char, self.read))
                    }
                },
                b')' if self.flags & SQUOTE == 0 => self.process_level -= 1,
                b';'  if (self.flags & (SQUOTE + DQUOTE) == 0) && self.process_level == 0 && self.array_process_level == 0 => {
                    return match error {
                        Some(error) => Some(Err(error)),
                        None        => Some(Ok(self.data[start..self.read-1].trim()))
                    };
                },
                b'#' if self.flags & (SQUOTE + DQUOTE) == 0 && self.process_level == 0 && self.array_process_level == 0 => {
                    let output = self.data[start..self.read-1].trim();
                    self.read = self.data.len();
                    return match error {
                        Some(error) => Some(Err(error)),
                        None        => Some(Ok(output))
                    };
                },
                b' ' if else_found => {
                    let output = &self.data[else_pos..self.read-1].trim();
                    if !output.is_empty() {
                        if "if" != *output {
                            self.read = else_pos;
                            return Some(Ok("else"));
                        }
                    }
                    else_found = false;
                }
                b' ' if !first_arg_found => {
                    let output = &self.data[start..self.read-1].trim();
                    if !output.is_empty() {
                        match *output {
                            "else" => {
                                else_found = true;
                                else_pos = self.read;
                            },
                            _ => first_arg_found = true
                        }
                    }
                }
                _ => ()
            }
            self.flags &= u16::MAX ^ (COMM_1 + COMM_2);
        }

        if start == self.read {
            None
        } else {
            self.read = self.data.len();
            match error {
                Some(error) => Some(Err(error)),
                None if self.process_level != 0 || self.array_process_level != 0 ||
                    self.array_level != 0 =>
                {
                    Some(Err(StatementError::UnterminatedSubshell))
                },
                None if self.flags & METHOD != 0 => Some(Err(StatementError::UnterminatedMethod)),
                None if self.flags & VBRACE != 0 => Some(Err(StatementError::UnterminatedBracedVar)),
                None if self.brace_level != 0    => Some(Err(StatementError::UnterminatedBrace)),
                None => {
                    let output = self.data[start..].trim();
                    match output.as_bytes()[0] {
                        b'>' | b'<' | b'^' => Some(Err(StatementError::ExpectedCommandButFound("redirection"))),
                        b'|'               => Some(Err(StatementError::ExpectedCommandButFound("pipe"))),
                        b'&'               => Some(Err(StatementError::ExpectedCommandButFound("&"))),
                        b'*' | b'%' | b'?' | b'{' | b'}' => Some(Err(StatementError::IllegalCommandName(output))),
                        _                                => Some(Ok(output))
                    }
                }
            }
        }
    }
}

#[test]
fn syntax_errors() {
    let command = "echo (echo one); echo $((echo one); echo ) two; echo $(echo one";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results[0], Err(StatementError::InvalidCharacter('(', 6)));
    assert_eq!(results[1], Err(StatementError::InvalidCharacter('(', 25)));
    assert_eq!(results[2], Err(StatementError::InvalidCharacter(')', 42)));
    assert_eq!(results[3], Err(StatementError::UnterminatedSubshell));
    assert_eq!(results.len(), 4);

    let command = ">echo";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results[0], Err(StatementError::ExpectedCommandButFound("redirection")));
    assert_eq!(results.len(), 1);
}

#[test]
fn methods() {
    let command = "echo $join(array, ', '); echo @join(var, ', ')";
    let statements = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(statements[0], Ok("echo $join(array, ', ')"));
    assert_eq!(statements[1], Ok("echo @join(var, ', ')"));
    assert_eq!(statements.len(), 2);
}

#[test]
fn processes() {
    let command = "echo $(seq 1 10); echo $(seq 1 10)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok("echo $(seq 1 10)"));
    }
}

#[test]
fn array_processes() {
    let command = "echo @[echo one; sleep 1]; echo @[echo one; sleep 1]";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok("echo @[echo one; sleep 1]"));
    }
}

#[test]
fn process_with_statements() {
    let command = "echo $(seq 1 10; seq 1 10)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok(command));
    }
}

#[test]
fn quotes() {
    let command = "echo \"This ;'is a test\"; echo 'This ;\" is also a test'";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok("echo \"This ;'is a test\""));
    assert_eq!(results[1], Ok("echo 'This ;\" is also a test'"));
}

#[test]
fn comments() {
    let command = "echo $(echo one # two); echo three # four";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok("echo $(echo one # two)"));
    assert_eq!(results[1], Ok("echo three"));
}

#[test]
fn nested_process() {
    let command = "echo $(echo one $(echo two) three)";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(command));

    let command = "echo $(echo $(echo one; echo two); echo two)";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(command));
}

#[test]
fn nested_array_process() {
    let command = "echo @[echo one @[echo two] three]";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(command));

    let command = "echo @[echo @[echo one; echo two]; echo two]";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(command));
}

#[test]
fn braced_variables() {
    let command = "echo ${foo}bar ${bar}baz ${baz}quux @{zardoz}wibble";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results, vec![Ok(command)]);
}
