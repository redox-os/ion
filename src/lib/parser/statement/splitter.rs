// TODO:
// - Rewrite this in the same style as shell_expand::words.
// - Validate syntax in methods

use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum LogicalOp {
    And,
    Or,
    None,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum Quotes {
    Single,
    Double,
    None,
}

#[derive(Debug, PartialEq)]
pub enum StatementError {
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

#[derive(Debug, PartialEq)]
pub enum StatementVariant<'a> {
    And(&'a str),
    Or(&'a str),
    Default(&'a str),
}

#[derive(Debug)]
pub struct StatementSplitter<'a> {
    data: &'a str,
    read: usize,
    start: usize,
    paren_level: u8,
    brace_level: u8,
    math_paren_level: i8,
    logical: LogicalOp,
    /// Set while parsing through an inline arithmetic expression, e.g. $((foo * bar / baz))
    math_expr: bool,
    skip: bool,
    vbrace: bool,
    method: bool,
    variable: bool,
    quotes: Quotes,
}

impl<'a> StatementSplitter<'a> {
    pub fn new(data: &'a str) -> Self {
        StatementSplitter {
            data,
            read: 0,
            start: 0,
            paren_level: 0,
            brace_level: 0,
            math_paren_level: 0,
            logical: LogicalOp::None,
            math_expr: false,
            skip: false,
            vbrace: false,
            method: false,
            variable: false,
            quotes: Quotes::None,
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
        let mut first_arg_found = false;
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
                b'\\' => self.skip = true,
                b'\'' => {
                    if self.quotes == Quotes::Single {
                        self.quotes = Quotes::None;
                    } else if self.quotes == Quotes::None {
                        self.variable = false;
                        self.quotes = Quotes::Single;
                    }
                }
                _ if self.quotes == Quotes::Single => {}
                // [^A-Za-z0-9_:,}]
                0...43 | 45...47 | 59...64 | 91...94 | 96 | 123...124 | 126...127
                    if self.vbrace =>
                {
                    // If we are just ending the braced section continue as normal
                    if error.is_none() {
                        error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                    }
                }
                // Toggle quotes and stop matching variables.
                b'"' if self.quotes == Quotes::Double => self.quotes = Quotes::None,
                b'"' => {
                    self.quotes = Quotes::Double;
                    self.variable = false;
                }
                // Array expansion
                b'@' | b'$' => self.variable = true,
                b'{' if [Some(b'$'), Some(b'@')].contains(&last) => self.vbrace = true,
                b'{' if self.quotes == Quotes::None => self.brace_level += 1,
                b'}' if self.vbrace => self.vbrace = false,
                b'}' if self.quotes == Quotes::None => {
                    if self.brace_level == 0 {
                        if error.is_none() {
                            error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                        }
                    } else {
                        self.brace_level -= 1;
                    }
                }
                b'(' if self.math_expr => self.math_paren_level += 1,
                b'(' if self.method || last == Some(b'$') => {
                    self.variable = false;
                    if let Some(&(_, b'(')) = bytes.peek() {
                        self.math_expr = true;
                        // The next character will always be a left paren in this branch;
                        self.math_paren_level = -1;
                    } else {
                        self.paren_level += 1;
                    }
                }
                b'(' if last == Some(b'@') => self.paren_level += 1,
                b'(' if self.variable => {
                    self.method = true;
                    self.variable = false;
                }
                b'(' if error.is_none() && self.quotes == Quotes::None => {
                    error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                }
                b')' if self.math_expr => {
                    if self.math_paren_level == 0 {
                        match bytes.peek() {
                            Some(&(_, b')')) => {
                                self.math_expr = false;
                                self.skip = true;
                            }
                            Some(&(_, next)) if error.is_none() => {
                                error = Some(StatementError::InvalidCharacter(next as char, i + 1));
                            }
                            None if error.is_none() => {
                                error = Some(StatementError::UnterminatedArithmetic)
                            }
                            _ => {}
                        }
                    } else {
                        self.math_paren_level -= 1;
                    }
                }
                b')' if self.method && self.paren_level == 0 => {
                    self.method = false;
                }
                b')' if self.paren_level == 0 => {
                    if error.is_none() && self.quotes == Quotes::None {
                        error = Some(StatementError::InvalidCharacter(character as char, i + 1))
                    }
                }
                b')' => self.paren_level -= 1,
                b';' if self.quotes == Quotes::None && self.paren_level == 0 => {
                    let statement = self.get_statement(start, i);
                    self.logical = LogicalOp::None;

                    self.read = i + 1;
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    };
                }
                b'&' | b'|'
                    if self.quotes == Quotes::None
                        && self.paren_level == 0
                        && last == Some(character) =>
                {
                    // Detecting if there is a 2nd `&` character
                    let statement = self.get_statement(start, i - 1);
                    self.logical = if character == b'&' { LogicalOp::And } else { LogicalOp::Or };
                    self.read = i + 1;
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    };
                }
                b' ' if !first_arg_found => match self.data[start..i].trim() {
                    "else" => {
                        if self.data.len() < i + 2 || &self.data[i + 1..=i + 2] != "if" {
                            self.logical = LogicalOp::None;
                            self.read = i + 1;
                            return Some(Ok(StatementVariant::Default("else")));
                        }
                    }
                    "" => {}
                    _ => first_arg_found = true,
                },
                // [^A-Za-z0-9_]
                0...47 | 58...64 | 91...94 | 96 | 123...127 => self.variable = false,
                _ => {}
            }
            last = Some(character);
        }

        self.read = self.data.len();
        error.map(Err).or_else(|| {
            if self.paren_level != 0 {
                Some(Err(StatementError::UnterminatedSubshell))
            } else if self.method {
                Some(Err(StatementError::UnterminatedMethod))
            } else if self.vbrace {
                Some(Err(StatementError::UnterminatedBracedVar))
            } else if self.brace_level != 0 {
                Some(Err(StatementError::UnterminatedBrace))
            } else if self.math_expr {
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
