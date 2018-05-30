// TODO:
// - Rewrite this in the same style as shell_expand::words.
// - Validate syntax in methods

use std::{
    cmp::max, fmt::{self, Display, Formatter}, u16,
};

bitflags! {
    pub struct Flags : u16 {
        const DQUOTE = 1;
        const COMM_1 = 2;
        const COMM_2 = 4;
        const VBRACE = 8;
        const ARRAY  = 16;
        const VARIAB = 32;
        const METHOD = 64;
        /// Set while parsing through an inline arithmetic expression, e.g. $((foo * bar / baz))
        const MATHEXPR = 128;
        const POST_MATHEXPR = 256;
        const AND = 512;
        const OR = 1024;
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum StatementError {
    IllegalCommandName(String),
    InvalidCharacter(char, usize),
    UnterminatedSubshell,
    UnterminatedBracedVar,
    UnterminatedBrace,
    UnterminatedMethod,
    UnterminatedArithmetic,
    ExpectedCommandButFound(&'static str),
}

impl Display for StatementError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            StatementError::IllegalCommandName(ref command) => {
                writeln!(f, "illegal command name: {}", command)
            }
            StatementError::InvalidCharacter(character, position) => writeln!(
                f,
                "syntax error: '{}' at position {} is out of place",
                character, position
            ),
            StatementError::UnterminatedSubshell => {
                writeln!(f, "syntax error: unterminated subshell")
            }
            StatementError::UnterminatedBrace => writeln!(f, "syntax error: unterminated brace"),
            StatementError::UnterminatedBracedVar => {
                writeln!(f, "syntax error: unterminated braced var")
            }
            StatementError::UnterminatedMethod => writeln!(f, "syntax error: unterminated method"),
            StatementError::UnterminatedArithmetic => {
                writeln!(f, "syntax error: unterminated arithmetic subexpression")
            }
            StatementError::ExpectedCommandButFound(element) => {
                writeln!(f, "expected command, but found {}", element)
            }
        }
    }
}

/// Returns true if the byte matches [^A-Za-z0-9_]
fn is_invalid(byte: u8) -> bool {
    byte <= 47
        || (byte >= 58 && byte <= 64)
        || (byte >= 91 && byte <= 94)
        || byte == 96
        || (byte >= 123 && byte <= 127)
}

#[derive(Debug, PartialEq)]
pub(crate) enum StatementVariant<'a> {
    And(&'a str),
    Or(&'a str),
    Default(&'a str),
}

pub(crate) struct StatementSplitter<'a> {
    data:             &'a str,
    read:             usize,
    start:            usize,
    flags:            Flags,
    a_level:          u8,
    ap_level:         u8,
    p_level:          u8,
    brace_level:      u8,
    math_paren_level: i8,
}

impl<'a> StatementSplitter<'a> {
    fn single_quote<B: Iterator<Item = u8>>(&mut self, bytes: &mut B) -> usize {
        let mut read = 0;
        while let Some(character) = bytes.next() {
            read += 1;
            if character == b'\\' {
                read += 1;
                bytes.next();
            } else if character == b'\'' {
                break;
            }
        }
        read
    }

    pub(crate) fn new(data: &'a str) -> Self {
        StatementSplitter {
            data,
            read: 0,
            start: 0,
            flags: Flags::empty(),
            a_level: 0,
            ap_level: 0,
            p_level: 0,
            brace_level: 0,
            math_paren_level: 0,
        }
    }

    fn get_statement(&mut self, flags: Flags) -> StatementVariant<'a> {
        self.flags.insert(flags);
        match self.flags {
            Flags::AND => {
                StatementVariant::And(&self.data[..self.read - 1].trim())
            }
            Flags::OR => {
                StatementVariant::Or(&self.data[..self.read - 1].trim())
            }
            _ => {
                StatementVariant::Default(&self.data[self.start..self.read - 1].trim())
            }
        }
    }
}

impl<'a> Iterator for StatementSplitter<'a> {
    type Item = Result<StatementVariant<'a>, StatementError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.start = self.read;
        let mut first_arg_found = false;
        let mut else_found = false;
        let mut else_pos = 0;
        let mut error = None;
        let c = self.data;
        let mut bytes = c.bytes().skip(self.read);
        while let Some(character) = bytes.next() {
            self.read += 1;
            match character {
                b'\\' => {
                    self.read += 1;
                    bytes.next();
                }
                _ if self.flags.contains(Flags::POST_MATHEXPR) => {
                    self.flags -= Flags::POST_MATHEXPR;
                }
                // [^A-Za-z0-9_:,}]
                0...43 | 45...47 | 59...64 | 91...94 | 96 | 123...124 | 126...127
                    if self.flags.contains(Flags::VBRACE) =>
                {
                    // If we are just ending the braced section continue as normal
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(
                            character as char,
                            self.read,
                        ))
                    }
                }
                b'\'' if !self.flags.contains(Flags::DQUOTE) => {
                    self.flags -= Flags::VARIAB | Flags::ARRAY;
                    self.read += self.single_quote(&mut bytes);
                }
                // Toggle Flags::DQUOTE and disable Flags::VARIAB + Flags::ARRAY.
                b'"' => self.flags = (self.flags ^ Flags::DQUOTE) - (Flags::VARIAB | Flags::ARRAY),
                // Disable Flags::COMM_1 and enable Flags::COMM_2 + Flags::ARRAY.
                b'@' => {
                    self.flags = (self.flags - Flags::COMM_1) | (Flags::COMM_2 | Flags::ARRAY);
                    continue;
                }
                b'$' => {
                    self.flags = (self.flags - Flags::COMM_2) | (Flags::COMM_1 | Flags::VARIAB);
                    continue;
                }
                b'{' if self.flags.intersects(Flags::COMM_1 | Flags::COMM_2) => {
                    self.flags |= Flags::VBRACE
                }
                b'{' if !self.flags.contains(Flags::DQUOTE) => self.brace_level += 1,
                b'}' if self.flags.contains(Flags::VBRACE) => self.flags.toggle(Flags::VBRACE),
                b'}' if !self.flags.contains(Flags::DQUOTE) => if self.brace_level == 0 {
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(
                            character as char,
                            self.read,
                        ))
                    }
                } else {
                    self.brace_level -= 1;
                },
                b'(' if self.flags.contains(Flags::MATHEXPR) => {
                    self.math_paren_level += 1;
                }
                b'(' if !self
                    .flags
                    .intersects(Flags::COMM_1 | Flags::VARIAB | Flags::ARRAY) =>
                {
                    if error.is_none() && !self.flags.contains(Flags::DQUOTE) {
                        error = Some(StatementError::InvalidCharacter(
                            character as char,
                            self.read,
                        ))
                    }
                }
                b'(' if self.flags.intersects(Flags::COMM_1 | Flags::METHOD) => {
                    self.flags -= Flags::VARIAB | Flags::ARRAY;
                    if self.data.as_bytes()[self.read] == b'(' {
                        self.flags = (self.flags - Flags::COMM_1) | Flags::MATHEXPR;
                        // The next character will always be a left paren in this branch;
                        self.math_paren_level = -1;
                    } else {
                        self.p_level += 1;
                    }
                }
                b'(' if self.flags.contains(Flags::COMM_2) => {
                    self.ap_level += 1;
                }
                b'(' if self.flags.intersects(Flags::VARIAB | Flags::ARRAY) => {
                    self.flags = (self.flags - (Flags::VARIAB | Flags::ARRAY)) | Flags::METHOD;
                }
                b')' if self.flags.contains(Flags::MATHEXPR) => if self.math_paren_level == 0 {
                    if self.data.as_bytes().len() <= self.read {
                        if error.is_none() {
                            error = Some(StatementError::UnterminatedArithmetic)
                        }
                    } else {
                        let next_character = self.data.as_bytes()[self.read] as char;
                        if next_character == ')' {
                            self.flags = (self.flags - Flags::MATHEXPR) | Flags::POST_MATHEXPR;
                        } else if error.is_none() {
                            error =
                                Some(StatementError::InvalidCharacter(next_character, self.read));
                        }
                    }
                } else {
                    self.math_paren_level -= 1;
                },
                b')' if self.flags.contains(Flags::METHOD) && self.p_level == 0 => {
                    self.flags ^= Flags::METHOD;
                }
                b')' if self.p_level + self.ap_level == 0 => {
                    if error.is_none() && !self.flags.contains(Flags::DQUOTE) {
                        error = Some(StatementError::InvalidCharacter(
                            character as char,
                            self.read,
                        ))
                    }
                }
                b')' if self.p_level != 0 => self.p_level -= 1,
                b')' => self.ap_level -= 1,
                b';' if !self.flags.contains(Flags::DQUOTE)
                    && self.p_level == 0
                    && self.ap_level == 0 =>
                {
                    let statement = self.get_statement(Flags::empty());
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    }
                }
                b'&' if !self.flags.contains(Flags::DQUOTE)
                    && self.p_level == 0
                    && self.ap_level == 0 =>
                {
                    if self.data.len() > self.read {
                        match self.data.as_bytes()[self.read] {
                            b'&' => {
                                let statement = self.get_statement(Flags::AND);
                                self.read = 0;
                                return match error {
                                    Some(error) => Some(Err(error)),
                                    None => Some(Ok(statement)),
                                };
                            }
                            _ => (),
                        }
                    }
                }
                b'|' if !self.flags.contains(Flags::DQUOTE)
                    && self.p_level == 0
                    && self.ap_level == 0 =>
                {
                    if self.data.len() > self.read {
                        match self.data.as_bytes()[self.read] {
                            b'|' => {
                                let statement = self.get_statement(Flags::OR);
                                self.read = 0;
                                return match error {
                                    Some(error) => Some(Err(error)),
                                    None => Some(Ok(statement)),
                                };
                            }
                            _ => (),
                        }
                    }
                }

                b'#' if self.read == 1
                    || (!self.flags.contains(Flags::DQUOTE) && self.p_level + self.ap_level == 0
                        && match self.data.as_bytes()[self.read - 2] {
                            b' ' | b'\t' => true,
                            _ => false,
                        }) =>
                {
                    let statement = self.get_statement(Flags::empty());
                    self.read = self.data.len();
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    };
                }
                b' ' if else_found => {
                    let output = &self.data[else_pos..self.read - 1].trim();
                    if !output.is_empty() {
                        if "if" != *output {
                            self.read = else_pos;
                            return Some(Ok(StatementVariant::Default("else")));
                        }
                    }
                    else_found = false;
                }
                b' ' if !first_arg_found => {
                    let output = &self.data[self.start..self.read - 1].trim();
                    if !output.is_empty() {
                        match *output {
                            "else" => {
                                else_found = true;
                                else_pos = self.read;
                            }
                            _ => first_arg_found = true,
                        }
                    }
                }
                // [^A-Za-z0-9_]
                byte => if self.flags.intersects(Flags::VARIAB | Flags::ARRAY) {
                    self.flags -= if is_invalid(byte) {
                        Flags::VARIAB | Flags::ARRAY
                    } else {
                        Flags::empty()
                    };
                },
            }
            self.flags -= Flags::COMM_1 | Flags::COMM_2;
        }

        if self.start == self.read {
            None
        } else {
            self.read = self.data.len();
            match error {
                Some(error) => Some(Err(error)),
                None if self.p_level != 0 || self.ap_level != 0 || self.a_level != 0 => {
                    Some(Err(StatementError::UnterminatedSubshell))
                }
                None if self.flags.contains(Flags::METHOD) => {
                    Some(Err(StatementError::UnterminatedMethod))
                }
                None if self.flags.contains(Flags::VBRACE) => {
                    Some(Err(StatementError::UnterminatedBracedVar))
                }
                None if self.brace_level != 0 => Some(Err(StatementError::UnterminatedBrace)),
                None if self.flags.contains(Flags::MATHEXPR) => {
                    Some(Err(StatementError::UnterminatedArithmetic))
                }
                None => {
                    let output = self.data[self.start..].trim();
                    if output.is_empty() {
                        return Some(Ok(StatementVariant::Default(output)));
                    }
                    match output.as_bytes()[0] {
                        b'>' | b'<' | b'^' => {
                            Some(Err(StatementError::ExpectedCommandButFound("redirection")))
                        }
                        b'|' => Some(Err(StatementError::ExpectedCommandButFound("pipe"))),
                        b'&' => Some(Err(StatementError::ExpectedCommandButFound("&"))),
                        b'*' | b'%' | b'?' | b'{' | b'}' => Some(Err(
                            StatementError::IllegalCommandName(String::from(output)),
                        )),
                        _ => Some(Ok(StatementVariant::Default(output))),
                    }
                }
            }
        }
    }
}

#[test]
fn syntax_errors() {
    let command = "echo (echo one); echo $( (echo one); echo ) two; echo $(echo one";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results[0], Err(StatementError::InvalidCharacter('(', 6)));
    assert_eq!(results[1], Err(StatementError::InvalidCharacter('(', 26)));
    assert_eq!(results[2], Err(StatementError::InvalidCharacter(')', 43)));
    assert_eq!(results[3], Err(StatementError::UnterminatedSubshell));
    assert_eq!(results.len(), 4);

    let command = ">echo";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(
        results[0],
        Err(StatementError::ExpectedCommandButFound("redirection"))
    );
    assert_eq!(results.len(), 1);

    let command = "echo $((foo bar baz)";
    let results = StatementSplitter::new(String::from(command)).collect::<Vec<_>>();
    assert_eq!(results[0], Err(StatementError::UnterminatedArithmetic));
    assert_eq!(results.len(), 1);
}

#[test]
fn methods() {
    let command = "echo $join(array, ', '); echo @join(var, ', ')";
    let statements = StatementSplitter::new(String::from(command)).collect::<Vec<_>>();
    assert_eq!(statements[0], Ok(String::from("echo $join(array, ', ')")));
    assert_eq!(statements[1], Ok(String::from("echo @join(var, ', ')")));
    assert_eq!(statements.len(), 2);
}

#[test]
fn processes() {
    let command = "echo $(seq 1 10); echo $(seq 1 10)";
    for statement in StatementSplitter::new(String::from(command)) {
        assert_eq!(statement, Ok(String::from("echo $(seq 1 10)")));
    }
}

#[test]
fn array_processes() {
    let command = "echo @(echo one; sleep 1); echo @(echo one; sleep 1)";
    for statement in StatementSplitter::new(String::from(command)) {
        assert_eq!(statement, Ok(String::from("echo @(echo one; sleep 1)")));
    }
}

#[test]
fn process_with_statements() {
    let command = "echo $(seq 1 10; seq 1 10)";
    for statement in StatementSplitter::new(String::from(command)) {
        assert_eq!(statement, Ok(String::from(command)));
    }
}

#[test]
fn quotes() {
    let command = "echo \"This ;'is a test\"; echo 'This ;\" is also a test'";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok(String::from("echo \"This ;'is a test\"")));
    assert_eq!(
        results[1],
        Ok(String::from("echo 'This ;\" is also a test'"))
    );
}

#[test]
fn comments() {
    let command = "echo $(echo one # two); echo three # four";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok(String::from("echo $(echo one # two)")));
    assert_eq!(results[1], Ok(String::from("echo three")));
}

#[test]
fn nested_process() {
    let command = "echo $(echo one $(echo two) three)";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(String::from(command)));

    let command = "echo $(echo $(echo one; echo two); echo two)";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(String::from(command)));
}

#[test]
fn nested_array_process() {
    let command = "echo @(echo one @(echo two) three)";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(String::from(command)));

    let command = "echo @(echo @(echo one; echo two); echo two)";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(String::from(command)));
}

#[test]
fn braced_variables() {
    let command = "echo ${foo}bar ${bar}baz ${baz}quux @{zardoz}wibble";
    let results = StatementSplitter::new(String::from(command))
        .collect::<Vec<Result<String, StatementError>>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results, vec![Ok(String::from(command))]);
}
