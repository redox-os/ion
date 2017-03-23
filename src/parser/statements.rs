use std::io::{self, Write};
use flow_control::Statement;
use super::peg::parse;

const SQUOTE: u8 = 1;
const DQUOTE: u8 = 2;
const BACKSL: u8 = 4;
const COMM_1: u8 = 8;

#[derive(Debug, PartialEq)]
pub enum StatementError {
    InvalidCharacter(char, usize),
    UnterminatedSubshell
}

pub fn check_statement(statement: Result<&str, StatementError>) -> Option<Statement> {
    match statement {
        Ok(statement) => Some(parse(statement)),
        Err(err) => {
            let stderr = io::stderr();
            match err {
                StatementError::InvalidCharacter(character, position) => {
                    let _ = writeln!(stderr.lock(),
                        "ion: syntax error: '{}' at position {} is out of place",
                        character, position);
                },
                StatementError::UnterminatedSubshell => {
                    let _ = writeln!(stderr.lock(), "ion: syntax error: unterminated subshell");
                }
            }
            None
        }
    }
}

pub struct StatementSplitter<'a> {
    data:  &'a str,
    read:  usize,
    flags: u8,
    process_level: u8,
    // brace_level: u8,
}

impl<'a> StatementSplitter<'a> {
    pub fn new(data: &'a str) -> StatementSplitter<'a> {
        StatementSplitter { data: data, read: 0, flags: 0, process_level: 0 }
    }
}

impl<'a> Iterator for StatementSplitter<'a> {
    type Item = Result<&'a str, StatementError>;
    fn next(&mut self) -> Option<Result<&'a str, StatementError>> {
        let start = self.read;
        let mut error = None;
        for character in self.data.bytes().skip(self.read) {
            self.read += 1;
            match character {
                _ if self.flags & BACKSL != 0     => self.flags ^= BACKSL,
                b'\\'                             => self.flags ^= BACKSL,
                b'\'' if self.flags & DQUOTE == 0 => self.flags ^= SQUOTE,
                b'"'  if self.flags & SQUOTE == 0 => self.flags ^= DQUOTE,
                b'$'  if self.flags & SQUOTE == 0 => { self.flags |= COMM_1; continue },
                b'('  if self.flags & COMM_1 == 0 => {
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(character as char, self.read))
                    }
                },
                b'(' if self.flags & COMM_1 != 0  && self.flags & SQUOTE == 0 => {
                    self.process_level += 1
                },
                b')' if self.process_level == 0 && self.flags & SQUOTE == 0 => {
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(character as char, self.read))
                    }
                },
                b')' if self.flags & SQUOTE == 0 => self.process_level -= 1,
                b';'  if (self.flags & (SQUOTE + DQUOTE) == 0) && self.process_level == 0 => {
                    return match error {
                        Some(error) => Some(Err(error)),
                        None        => Some(Ok(self.data[start..self.read-1].trim()))
                    };
                },
                b'#' if self.flags & (SQUOTE + DQUOTE) == 0 && self.process_level == 0 => {
                    let output = self.data[start..self.read-1].trim();
                    self.read = self.data.len();
                    return match error {
                        Some(error) => Some(Err(error)),
                        None        => Some(Ok(output))
                    };
                },
                _ => ()
            }
            self.flags &= 255 ^ COMM_1;
        }

        if start == self.read {
            None
        } else {
            self.read = self.data.len();
            match error {
                Some(error) => Some(Err(error)),
                None if self.process_level != 0 => Some(Err(StatementError::UnterminatedSubshell)),
                None => Some(Ok(self.data[start..].trim()))
            }
        }
    }
}

#[test]
fn statements_with_syntax_errors() {
    let command = "echo (echo one); echo $((echo one); echo ) two; echo $(echo one";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 4);
    assert_eq!(results[0], Err(StatementError::InvalidCharacter('(', 6)));
    assert_eq!(results[1], Err(StatementError::InvalidCharacter('(', 25)));
    assert_eq!(results[2], Err(StatementError::InvalidCharacter(')', 42)));
    assert_eq!(results[3], Err(StatementError::UnterminatedSubshell));
}

#[test]
fn statements_with_processes() {
    let command = "echo $(seq 1 10); echo $(seq 1 10)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok("echo $(seq 1 10)"));
    }
}

#[test]
fn statements_process_with_statements() {
    let command = "echo $(seq 1 10; seq 1 10)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok(command));
    }
}

#[test]
fn statements_with_quotes() {
    let command = "echo \"This ;'is a test\"; echo 'This ;\" is also a test'";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok("echo \"This ;'is a test\""));
    assert_eq!(results[1], Ok("echo 'This ;\" is also a test'"));
}

#[test]
fn statements_with_comments() {
    let command = "echo $(echo one # two); echo three # four";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok("echo $(echo one # two)"));
    assert_eq!(results[1], Ok("echo three"));
}

#[test]
fn statements_with_process_recursion() {
    let command = "echo $(echo one $(echo two) three)";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(command));

    let command = "echo $(echo $(echo one; echo two); echo two)";
    let results = StatementSplitter::new(command).collect::<Vec<Result<&str, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(command));
}
