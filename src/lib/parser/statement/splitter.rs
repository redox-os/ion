// TODO:
// - Rewrite this in the same style as shell_expand::words.
// - Validate syntax in methods

use err_derive::Error;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum LogicalOp {
    And,
    Or,
    None,
}

#[derive(Debug, PartialEq, Error)]
pub enum StatementError {
    #[error(display = "illegal command name: {}", _0)]
    IllegalCommandName(String),
    #[error(display = "syntax error: '{}' at position {} is out of place", _0, _1)]
    InvalidCharacter(char, usize),
    #[error(display = "syntax error: unterminated subshell")]
    UnterminatedSubshell,
    #[error(display = "syntax error: unterminated brace")]
    UnterminatedBracedVar,
    #[error(display = "syntax error: unterminated braced var")]
    UnterminatedBrace,
    #[error(display = "syntax error: unterminated method")]
    UnterminatedMethod,
    #[error(display = "syntax error: unterminated arithmetic subexpression")]
    UnterminatedArithmetic,
    #[error(display = "expected command, but found {}", _0)]
    ExpectedCommandButFound(&'static str),
}

#[derive(Debug, PartialEq)]
pub enum StatementVariant<'a> {
    And(&'a str),
    Or(&'a str),
    Default(&'a str),
}

#[derive(Debug)]
pub struct StatementSplitter<'a> {
    data:             &'a str,
    read:             usize,
    paren_level:      u8,
    brace_level:      u8,
    math_paren_level: i8,
    logical:          LogicalOp,
    skip:             bool,
    vbrace:           bool,
    variable:         bool,
    quotes:           bool,
}

impl<'a> StatementSplitter<'a> {
    pub fn new(data: &'a str) -> Self {
        StatementSplitter {
            data,
            read: 0,
            paren_level: 0,
            brace_level: 0,
            math_paren_level: 0,
            logical: LogicalOp::None,
            skip: false,
            vbrace: false,
            variable: false,
            quotes: false,
        }
    }

    fn get_statement(&self, start: usize, end: usize) -> StatementVariant<'a> {
        if self.logical == LogicalOp::And {
            StatementVariant::And(&self.data[start + 1..end].trim())
        } else if self.logical == LogicalOp::Or {
            StatementVariant::Or(&self.data[start + 1..end].trim())
        } else {
            StatementVariant::Default(&self.data[start..end].trim())
        }
    }

    fn get_statement_from(&self, input: &'a str) -> StatementVariant<'a> {
        if self.logical == LogicalOp::And {
            StatementVariant::And(input)
        } else if self.logical == LogicalOp::Or {
            StatementVariant::Or(input)
        } else {
            StatementVariant::Default(input)
        }
    }
}

impl<'a> Iterator for StatementSplitter<'a> {
    type Item = Result<StatementVariant<'a>, StatementError>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.read;
        let mut error = None;
        let mut bytes = self.data.bytes().enumerate().skip(self.read).peekable();
        let mut last = None;

        bytes.peek()?;

        while let Some((i, character)) = bytes.next() {
            match character {
                _ if self.skip => {
                    self.skip = false;
                    last = None;
                    continue;
                }
                b'\'' if !self.quotes => {
                    self.variable = false;
                    bytes.find(|&(_, c)| c == b'\'');
                }
                b'\\' => self.skip = true,
                // [^A-Za-z0-9_:,}]
                0..=43 | 45..=47 | 59..=64 | 91..=94 | 96 | 123..=124 | 126..=127
                    if self.vbrace =>
                {
                    // If we are just ending the braced section continue as normal
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                    }
                }
                // Toggle quotes and stop matching variables.
                b'"' if self.quotes && self.paren_level == 0 => self.quotes = false,
                b'"' => {
                    self.quotes = true;
                    self.variable = false;
                }
                // Array expansion
                b'@' | b'$' => self.variable = true,
                b'{' if [Some(b'$'), Some(b'@')].contains(&last) => self.vbrace = true,
                b'(' if self.math_paren_level > 0 => self.math_paren_level += 1,
                b'(' if self.variable && last == Some(b'(') => {
                    self.math_paren_level = 1;
                    self.paren_level -= 1;
                }
                b'(' if self.variable => self.paren_level += 1,
                b'(' if error.is_none() && !self.quotes => {
                    error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                }
                b')' if self.math_paren_level == 1 => match bytes.peek() {
                    Some(&(_, b')')) => {
                        self.math_paren_level = 0;
                        self.skip = true;
                    }
                    Some(&(_, next)) if error.is_none() => {
                        error = Some(StatementError::InvalidCharacter(next as char, i + 1));
                    }
                    None if error.is_none() => error = Some(StatementError::UnterminatedArithmetic),
                    _ => {}
                },
                b'(' if self.math_paren_level != 0 => {
                    self.math_paren_level -= 1;
                }
                b')' if self.paren_level == 0 => {
                    if !self.variable && error.is_none() && !self.quotes {
                        error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                    }
                    self.variable = false;
                }
                b')' => self.paren_level -= 1,
                b'}' if self.vbrace => self.vbrace = false,
                // [^A-Za-z0-9_]
                0..=37 | 39..=47 | 58 | 60..=64 | 91..=94 | 96 | 126..=127 => self.variable = false,
                _ if self.quotes => {}
                b'{' => self.brace_level += 1,
                b'}' => {
                    if self.brace_level == 0 {
                        if error.is_none() {
                            error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                        }
                    } else {
                        self.brace_level -= 1;
                    }
                }
                b';' if self.paren_level == 0 => {
                    let statement = self.get_statement(start, i);
                    self.logical = LogicalOp::None;

                    self.read = i + 1;
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    };
                }
                b'&' | b'|' if self.paren_level == 0 && last == Some(character) => {
                    // Detecting if there is a 2nd `&` character
                    let statement = self.get_statement(start, i - 1);
                    self.logical = if character == b'&' { LogicalOp::And } else { LogicalOp::Or };
                    self.read = i + 1;
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    };
                }
                _ => {}
            }
            last = Some(character);
        }

        self.read = self.data.len();
        error.map(Err).or_else(|| {
            if self.paren_level != 0 && self.variable {
                Some(Err(StatementError::UnterminatedMethod))
            } else if self.paren_level != 0 {
                Some(Err(StatementError::UnterminatedSubshell))
            } else if self.vbrace {
                Some(Err(StatementError::UnterminatedBracedVar))
            } else if self.brace_level != 0 {
                Some(Err(StatementError::UnterminatedBrace))
            } else if self.math_paren_level != 0 {
                Some(Err(StatementError::UnterminatedArithmetic))
            } else {
                let output = self.data[start..].trim();
                output.as_bytes().get(0).map(|c| match c {
                    b'>' | b'<' | b'^' => {
                        Err(StatementError::ExpectedCommandButFound("redirection"))
                    }
                    b'|' => Err(StatementError::ExpectedCommandButFound("pipe")),
                    b'&' => Err(StatementError::ExpectedCommandButFound("&")),
                    b'*' | b'%' | b'?' | b'{' | b'}' => {
                        Err(StatementError::IllegalCommandName(String::from(output)))
                    }
                    _ => {
                        let stmt = self.get_statement_from(output);
                        self.logical = LogicalOp::None;
                        Ok(stmt)
                    }
                })
            }
        })
    }
}

#[test]
fn syntax_errors() {
    let command = "echo (echo one); echo $( (echo one); echo ) two; echo $(echo one";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(StatementError::InvalidCharacter('(', 6)));
    assert_eq!(results[1], Err(StatementError::InvalidCharacter('(', 26)));
    assert_eq!(results[2], Err(StatementError::InvalidCharacter(')', 43)));
    assert_eq!(results[3], Err(StatementError::UnterminatedSubshell));
    assert_eq!(results.len(), 4);

    let command = ">echo";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(StatementError::ExpectedCommandButFound("redirection")));
    assert_eq!(results.len(), 1);

    let command = "echo $((foo bar baz)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(StatementError::UnterminatedArithmetic));
    assert_eq!(results.len(), 1);
}

#[test]
fn methods() {
    let command = "echo $join(array, ', '); echo @join(var, ', ')";
    let statements = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(statements[0], Ok(StatementVariant::Default("echo $join(array, ', ')")));
    assert_eq!(statements[1], Ok(StatementVariant::Default("echo @join(var, ', ')")));
    assert_eq!(statements.len(), 2);
}

#[test]
fn processes() {
    let command = "echo $(seq 1 10); echo $(seq 1 10)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok(StatementVariant::Default("echo $(seq 1 10)")));
    }
}

#[test]
fn array_processes() {
    let command = "echo @(echo one; sleep 1); echo @(echo one; sleep 1)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok(StatementVariant::Default("echo @(echo one; sleep 1)")));
    }
}

#[test]
fn process_with_statements() {
    let command = "echo $(seq 1 10; seq 1 10)";
    for statement in StatementSplitter::new(command) {
        assert_eq!(statement, Ok(StatementVariant::Default(command)));
    }
}

#[test]
fn quotes() {
    let command = "echo \"This ;'is a test\"; echo 'This ;\" is also a test'";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok(StatementVariant::Default("echo \"This ;'is a test\"")));
    assert_eq!(results[1], Ok(StatementVariant::Default("echo 'This ;\" is also a test'")));
}

#[test]
fn nested_process() {
    let command = "echo $(echo one $(echo two) three)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));

    let command = "echo $(echo $(echo one; echo two); echo two)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
}

#[test]
fn nested_array_process() {
    let command = "echo @(echo one @(echo two) three)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));

    let command = "echo @(echo @(echo one; echo two); echo two)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
}

#[test]
fn braced_variables() {
    let command = "echo ${foo}bar ${bar}baz ${baz}quux @{zardoz}wibble";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
}

#[test]
fn variants() {
    let command = r#"echo "Hello!"; echo "How are you doing?" && echo "I'm just an ordinary test." || echo "Helping by making sure your code works right."; echo "Have a good day!""#;
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0], Ok(StatementVariant::Default(r#"echo "Hello!""#)));
    assert_eq!(results[1], Ok(StatementVariant::Default(r#"echo "How are you doing?""#)));
    assert_eq!(results[2], Ok(StatementVariant::And(r#"echo "I'm just an ordinary test.""#)));
    assert_eq!(
        results[3],
        Ok(StatementVariant::Or(r#"echo "Helping by making sure your code works right.""#))
    );
    assert_eq!(results[4], Ok(StatementVariant::Default(r#"echo "Have a good day!""#)));
}
