// TODO:
// - Rewrite this in the same style as shell_expand::words.
// - Validate syntax in methods

use super::Error;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum LogicalOp {
    And,
    Or,
    None,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StatementVariant<'a> {
    And(&'a str),
    Or(&'a str),
    Default(&'a str),
}

/// Split an input data into a set of statements
#[derive(Debug)]
pub struct StatementSplitter<'a> {
    data:                 &'a str,
    read:                 usize,
    paren_level:          i8,
    brace_level:          i8,
    square_bracket_level: i8,
    math_paren_level:     i8,
    logical:              LogicalOp,
    vbrace:               bool,
    variable:             bool,
    single_quotes:        bool,
    double_quotes:        bool,
}

impl<'a> StatementSplitter<'a> {
    /// Create a new statement splitter on data
    pub const fn new(data: &'a str) -> Self {
        Self {
            data,
            read: 0,
            paren_level: 0,
            brace_level: 0,
            square_bracket_level: 0,
            math_paren_level: 0,
            logical: LogicalOp::None,
            vbrace: false,
            variable: false,
            single_quotes: false,
            double_quotes: false,
        }
    }

    fn inside_quotes(&self) -> bool { return self.single_quotes || self.double_quotes; }

    fn get_statement(&self, statement: &'a str) -> StatementVariant<'a> {
        match self.logical {
            LogicalOp::And => StatementVariant::And(statement.trim()),
            LogicalOp::Or => StatementVariant::Or(statement.trim()),
            LogicalOp::None => StatementVariant::Default(statement.trim()),
        }
    }
}

impl<'a> Iterator for StatementSplitter<'a> {
    type Item = Result<StatementVariant<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.read;
        let mut error = None;
        let mut bytes = self.data.bytes().enumerate().skip(self.read).peekable();
        let mut skip = false;
        let mut last = None;

        bytes.peek()?;

        while let Some((i, character)) = bytes.next() {
            match character {
                _ if skip => {
                    skip = false;
                    last = None;
                    continue;
                }
                b'\\' => skip = true,
                _ if self.vbrace => {
                    // We are in `${}` or `@{}` block, variable must use
                    // the following charset : [^A-Za-z0-9_:,}]
                    match character {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b':' | b',' => (),
                        b'}' => {
                            self.vbrace = false;
                        }
                        _ => {
                            if error.is_none() {
                                error = Some(Error::InvalidCharacter(character as char, i + 1))
                            }
                        }
                    }
                }
                // Toggle quotes and stop matching variables.
                b'\'' if !self.double_quotes => {
                    self.single_quotes = !self.single_quotes;
                    self.variable = false;
                }
                b'"' if !self.single_quotes => {
                    self.double_quotes = !self.double_quotes;
                    self.variable = false;
                }
                // square brackets
                b'[' if !self.inside_quotes() => {
                    self.square_bracket_level += 1;
                }
                b']' if !self.inside_quotes() => {
                    self.square_bracket_level -= 1;
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
                b'(' if error.is_none() && !self.inside_quotes() => {
                    error = Some(Error::InvalidCharacter(character as char, i + 1))
                }
                b')' if self.math_paren_level == 1 => match bytes.peek() {
                    Some(&(_, b')')) => {
                        self.math_paren_level = 0;
                        skip = true;
                    }
                    Some(&(_, next)) if error.is_none() => {
                        error = Some(Error::InvalidCharacter(next as char, i + 2));
                    }
                    None | _ => {
                        if error.is_none() {
                            error = Some(Error::UnterminatedArithmetic);
                        }
                    }
                },
                b')' if self.paren_level == 0 => {
                    if !self.variable && error.is_none() && !self.inside_quotes() {
                        error = Some(Error::InvalidCharacter(character as char, i + 1))
                    }
                    self.variable = false;
                }
                b')' => self.paren_level -= 1,
                // [^A-Za-z0-9_]
                0..=37 | 39..=47 | 58 | 60..=64 | 91..=94 | 96 | 126..=127 => self.variable = false,
                _ if self.inside_quotes() => {}
                b'{' => self.brace_level += 1,
                b'}' => {
                    if self.brace_level == 0 {
                        if error.is_none() {
                            error = Some(Error::InvalidCharacter(character as char, i + 1))
                        }
                    } else {
                        self.brace_level -= 1;
                    }
                }
                b';' if self.paren_level == 0 => {
                    self.read = i + 1;
                    if start == i {
                        return Some(Err(Error::ExpectedCommandButFound(";")));
                    }
                    let statement = self.get_statement(&self.data[start..i]);
                    self.logical = LogicalOp::None;
                    return match error {
                        Some(error) => Some(Err(error)),
                        None => Some(Ok(statement)),
                    };
                }
                // Detecting if there is a 2nd `&` character
                b'&' | b'|' if self.paren_level == 0 && last == Some(character) => {
                    self.read = i + 1;
                    if start == i - 1 {
                        return {
                            if character == b'&' {
                                Some(Err(Error::ExpectedCommandButFound("&")))
                            } else {
                                Some(Err(Error::ExpectedCommandButFound("|")))
                            }
                        };
                    }
                    let statement = self.get_statement(&self.data[start..i - 1]);
                    self.logical = if character == b'&' { LogicalOp::And } else { LogicalOp::Or };
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
                Some(Err(Error::UnterminatedMethod))
            } else if self.paren_level != 0 {
                Some(Err(Error::UnterminatedSubshell))
            } else if self.vbrace {
                Some(Err(Error::UnterminatedBracedVar))
            } else if self.brace_level != 0 {
                Some(Err(Error::UnterminatedBrace))
            } else if self.math_paren_level != 0 {
                Some(Err(Error::UnterminatedArithmetic))
            } else if self.square_bracket_level != 0 {
                Some(Err(Error::UnterminatedSquareBracket))
            } else if self.single_quotes {
                Some(Err(Error::UnterminatedSingleQuotes))
            } else if self.double_quotes {
                Some(Err(Error::UnterminatedDoubleQuotes))
            } else {
                let output = self.data[start..].trim();
                output.as_bytes().get(0).map(|c| match c {
                    b'>' | b'<' | b'^' => Err(Error::ExpectedCommandButFound("redirection")),
                    b'|' => Err(Error::ExpectedCommandButFound("|")),
                    b'&' => Err(Error::ExpectedCommandButFound("&")),
                    _ => {
                        let stmt = self.get_statement(&self.data[start..]);
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
    assert_eq!(results[0], Err(Error::InvalidCharacter('(', 6)));
    assert_eq!(results[1], Err(Error::InvalidCharacter('(', 26)));
    assert_eq!(results[2], Err(Error::InvalidCharacter(')', 43)));
    assert_eq!(results[3], Err(Error::UnterminatedSubshell));
    assert_eq!(results.len(), 4);

    let command = "${+}";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::InvalidCharacter('+', 3)));
    assert_eq!(results.len(), 1);

    let command = ">echo";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::ExpectedCommandButFound("redirection")));
    assert_eq!(results.len(), 1);

    let command = "echo $((foo bar baz)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedArithmetic));
    assert_eq!(results.len(), 1);

    let command = "&&";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::ExpectedCommandButFound("&")));
    assert_eq!(results.len(), 1);

    let command = "||";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::ExpectedCommandButFound("|")));
    assert_eq!(results.len(), 1);

    let command = "ls &&||";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Err(Error::ExpectedCommandButFound("|")));
    assert_eq!(results.len(), 2);

    let command = "ls ||&&";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Err(Error::ExpectedCommandButFound("&")));
    assert_eq!(results.len(), 2);

    let command = "ls ;;";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Err(Error::ExpectedCommandButFound(";")));
    assert_eq!(results.len(), 2);

    let command = "{}}";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::InvalidCharacter('}', 3)));
    assert_eq!(results.len(), 1);

    let command = "ls @{+} &&";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::InvalidCharacter('+', 6)));
    assert_eq!(results.len(), 1);

    let command = "@{";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedBracedVar));
    assert_eq!(results.len(), 1);

    let command = "{";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedBrace));
    assert_eq!(results.len(), 1);

    let command = "@(";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedMethod));
    assert_eq!(results.len(), 1);

    let command = "@(()?";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::InvalidCharacter('?', 5)));
    assert_eq!(results.len(), 1);

    let command = "@((";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedArithmetic));
    assert_eq!(results.len(), 1);

    let command = "ls ; ls ; && ls; ls || ls && |";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[2], Ok(StatementVariant::Default("")));
    assert_eq!(results[3], Ok(StatementVariant::And("ls")));
    assert_eq!(results[4], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[5], Ok(StatementVariant::Or("ls")));
    assert_eq!(results[6], Err(Error::ExpectedCommandButFound("|")));
    assert_eq!(results.len(), 7);

    let command = "ls ; ls ; && ls; ls || ls && &";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[2], Ok(StatementVariant::Default("")));
    assert_eq!(results[3], Ok(StatementVariant::And("ls")));
    assert_eq!(results[4], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[5], Ok(StatementVariant::Or("ls")));
    assert_eq!(results[6], Err(Error::ExpectedCommandButFound("&")));

    let command = "let a b = one [two three four";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedSquareBracket));
    assert_eq!(results.len(), 1);

    let command = "let a b = one [two three four]]";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedSquareBracket));
    assert_eq!(results.len(), 1);

    let command = "echo '\"one\"' 'two''";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedSingleQuotes));
    assert_eq!(results.len(), 1);

    let command = "echo '\"one\"' 'two\"'\"";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Err(Error::UnterminatedDoubleQuotes));
    assert_eq!(results.len(), 1);
}

#[test]
fn arithmetic() {
    let command = "$((3 + 3))";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("$((3 + 3))")));
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
fn escaped_sequences() {
    let command = "ls \\&\\&";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls \\&\\&")));
    assert_eq!(results.len(), 1);

    let command = "\\@\\{ls\\}";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("\\@\\{ls\\}")));
    assert_eq!(results.len(), 1);
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
    assert_eq!(results[0], Ok(StatementVariant::Default("echo \"This ;'is a test\"")));
    assert_eq!(results[1], Ok(StatementVariant::Default("echo 'This ;\" is also a test'")));
    assert_eq!(results.len(), 2);
}

#[test]
fn nested_process() {
    let command = "echo $(echo one $(echo two) three)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
    assert_eq!(results.len(), 1);

    let command = "echo $(echo $(echo one; echo two); echo two)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
    assert_eq!(results.len(), 1);
}

#[test]
fn nested_array_process() {
    let command = "echo @(echo one @(echo two) three)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
    assert_eq!(results.len(), 1);

    let command = "echo @(echo @(echo one; echo two); echo two)";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
    assert_eq!(results.len(), 1);
}

#[test]
fn braced_variables() {
    let command = "echo ${foo}bar ${bar}baz ${baz}quux @{zardoz}wibble";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default(command)));
    assert_eq!(results.len(), 1);
}

#[test]
fn logical_operators() {
    let command = "ls && ls";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Ok(StatementVariant::And("ls")));
    assert_eq!(results.len(), 2);

    let command = "ls &&";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results.len(), 1);

    let command = "ls || ls";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results[1], Ok(StatementVariant::Or("ls")));
    assert_eq!(results.len(), 2);

    let command = "ls ||";
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default("ls")));
    assert_eq!(results.len(), 1);
}

#[test]
fn variants() {
    let command = r#"echo "Hello!"; echo "How are you doing?" && echo "I'm just an ordinary test." || echo "Helping by making sure your code works right."; echo "Have a good day!""#;
    let results = StatementSplitter::new(command).collect::<Vec<_>>();
    assert_eq!(results[0], Ok(StatementVariant::Default(r#"echo "Hello!""#)));
    assert_eq!(results[1], Ok(StatementVariant::Default(r#"echo "How are you doing?""#)));
    assert_eq!(results[2], Ok(StatementVariant::And(r#"echo "I'm just an ordinary test.""#)));
    assert_eq!(
        results[3],
        Ok(StatementVariant::Or(r#"echo "Helping by making sure your code works right.""#))
    );
    assert_eq!(results[4], Ok(StatementVariant::Default(r#"echo "Have a good day!""#)));
    assert_eq!(results.len(), 5);
}
