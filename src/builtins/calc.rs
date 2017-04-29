use std::io::{self, Write};
use std::error::Error;
use self::CalcError::*;

#[derive(Debug, Clone)]
pub enum Token {
    Plus,
    Minus,
    Divide,
    Multiply,
    Exponent,
    OpenParen,
    CloseParen,
    // TODO: Don't pass around a string when we can pass around a number
    Number(String),
}

impl Token {
    pub fn to_str(&self) -> &'static str {
        match *self {
            Token::Plus       => "Plus",
            Token::Minus      => "Minus",
            Token::Divide     => "Divide",
            Token::Multiply   => "Multiply",
            Token::Exponent   => "Exponent",
            Token::OpenParen  => "OpenParen",
            Token::CloseParen => "CloseParen",
            Token::Number(_)  => "Number",
        }
    }

    pub fn to_string(&self) -> String {
        self.to_str().to_owned()
    }
}

#[derive(Debug)]
pub enum CalcError {
    DivideByZero,
    InvalidNumber(String),
    InvalidOperator(char),
    UnrecognizedToken(String),
    UnexpectedToken(String, &'static str),
    UnexpectedEndOfInput,
    UnmatchedParenthesis,
    IO(io::Error),
}

impl From<CalcError> for String {
    fn from(data: CalcError) -> String {
        match data {
            DivideByZero                 => String::from("calc: attempted to divide by zero"),
            InvalidNumber(number)        => ["calc: invalid number: ", &number].concat(),
            InvalidOperator(character)   => format!("calc: invalid operator: {}", character),
            IO(error)                    => error.description().to_owned(),
            UnrecognizedToken(token)     => ["calc: unrecognized token: ", &token].concat(),
            UnexpectedToken(token, kind) => ["calc: unexpected ", kind, " token: ", &token].concat(),
            UnexpectedEndOfInput         => String::from("calc: unexpected end of input"),
            UnmatchedParenthesis         => String::from("calc: unmatched parenthesis")
        }
    }
}

#[derive(Clone,Debug)]
pub struct IntermediateResult {
    value: f64,
    tokens_read: usize,
}

impl IntermediateResult {
    fn new(value: f64, tokens_read: usize) -> Self {
        IntermediateResult {
            value: value,
            tokens_read: tokens_read,
        }
    }
}

pub trait OperatorFunctions {
    fn is_operator(self) -> bool;
    fn operator_type(self) -> Option<Token>;
}

impl OperatorFunctions for char {
    fn is_operator(self) -> bool {
        self == '+' ||
        self == '-' ||
        self == '*' ||
        self == '/' ||
        self == '^' ||
        self == '(' ||
        self == ')'
    }

    fn operator_type(self) -> Option<Token> {
        match self {
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '/' => Some(Token::Divide),
            '*' => Some(Token::Multiply),
            '^' => Some(Token::Exponent),
            '(' => Some(Token::OpenParen),
            ')' => Some(Token::CloseParen),
            _   => None
        }
    }
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, CalcError> {
    let mut tokens = Vec::with_capacity(input.len());

    // TODO: Not this. Modify to use iterator
    let chars: Vec<char> = input.chars().collect();

    let input_length = chars.len();
    let mut current_pos = 0;
    while current_pos < input_length {
        let c = chars[current_pos];
        if c.is_digit(10) || c == '.' {
            let token_string = consume_number(&chars[current_pos..]);
            current_pos += token_string.len();
            tokens.push(Token::Number(token_string));
        } else if c.is_operator() {
            tokens.push(c.operator_type().ok_or_else(|| InvalidOperator(c))?);
            current_pos += 1;
        } else if c.is_whitespace() {
            current_pos += 1;
        } else {
            let token_string = consume_until_new_token(&chars[current_pos..]);
            return Err(CalcError::UnrecognizedToken(token_string));
        }
    }
    Ok(tokens)
}

fn consume_number(input: &[char]) -> String {
    let mut number = String::with_capacity(input.len());
    let mut has_decimal_point = false;
    for &c in input {
        if c == '.' {
            if has_decimal_point {
                break;
            } else {
                number.push(c);
                has_decimal_point = true;
            }
        } else if c.is_digit(10) {
            number.push(c);
        } else {
            break;
        }
    }
    number
}

fn consume_until_new_token(input: &[char]) -> String {
    input.iter()
         .take_while(|c| !(c.is_whitespace() || c.is_operator() || c.is_digit(10)))
         .cloned()
         .collect()
}

// Addition and subtraction
pub fn e_expr(token_list: &[Token]) -> Result<IntermediateResult, CalcError> {
    let mut t1 = try!(t_expr(token_list));
    let mut index = t1.tokens_read;

    while index < token_list.len() {
        match token_list[index] {
            Token::Plus => {
                let t2 = try!(t_expr(&token_list[index+1..]));
                t1.value += t2.value;
                t1.tokens_read += t2.tokens_read + 1;
            }
            Token::Minus => {
                let t2 = try!(t_expr(&token_list[index+1..]));
                t1.value -= t2.value;
                t1.tokens_read += t2.tokens_read + 1;
            }
            Token::Number(ref n) => return Err(CalcError::UnexpectedToken(n.clone(),"operator")),
            _ => break,
        };
        index = t1.tokens_read;
    }
    Ok(t1)
}

// Multiplication and division
pub fn t_expr(token_list: &[Token]) -> Result<IntermediateResult, CalcError> {
    let mut f1 = try!(f_expr(token_list));
    let mut index = f1.tokens_read;

    while index < token_list.len() {
        match token_list[index] {
            Token::Multiply => {
                let f2 = try!(f_expr(&token_list[index+1..]));
                f1.value *= f2.value;
                f1.tokens_read += f2.tokens_read + 1;
            }
            Token::Divide => {
                let f2 = try!(f_expr(&token_list[index+1..]));
                if f2.value == 0.0 {
                    return Err(CalcError::DivideByZero);
                } else {
                    f1.value /= f2.value;
                    f1.tokens_read += f2.tokens_read + 1;
                }
            }
            Token::Number(ref n) => return Err(CalcError::UnexpectedToken(n.clone(),"operator")),
            _ => break,
        }
        index = f1.tokens_read;
    }
    Ok(f1)
}

// Exponentiation
pub fn f_expr(token_list: &[Token]) -> Result<IntermediateResult, CalcError> {
    let mut g1 = try!(g_expr(token_list));
    let mut index = g1.tokens_read;
    let token_len = token_list.len();
    while index < token_len {
        match token_list[index] {
            Token::Exponent => {
                let f = try!(f_expr(&token_list[index+1..]));
                g1.value = g1.value.powf(f.value);
                g1.tokens_read += f.tokens_read + 1;
            }
            Token::Number(ref n) => return Err(CalcError::UnexpectedToken(n.clone(),"operator")),
            _ => break,
        }
        index = g1.tokens_read;
    }
    Ok(g1)
}

// Numbers and parenthesized expressions
pub fn g_expr(token_list: &[Token]) -> Result<IntermediateResult, CalcError> {
    if !token_list.is_empty() {
        match token_list[0] {
            Token::Number(ref n) => {
                n.parse::<f64>()
                 .map_err(|_| CalcError::InvalidNumber(n.clone()))
                 .and_then(|num| Ok(IntermediateResult::new(num, 1)))
            }
            Token::Minus => {
                if token_list.len() > 1 {
                    if let Token::Number(ref n) = token_list[1] {
                        n.parse::<f64>()
                         .map_err(|_| CalcError::InvalidNumber(n.clone()))
                         .and_then(|num| Ok(IntermediateResult::new(-1.0 * num, 2)))
                    } else {
                        Err(CalcError::UnexpectedToken(token_list[1].to_string(), "number"))
                    }
                } else {
                    Err(CalcError::UnexpectedEndOfInput)
                }
            }
            Token::OpenParen => {
                let expr = e_expr(&token_list[1..]);
                match expr {
                    Ok(ir) => {
                        let close_paren = ir.tokens_read + 1;
                        if close_paren < token_list.len() {
                            match token_list[close_paren] {
                                Token::CloseParen => Ok(IntermediateResult::new(ir.value, close_paren+1)),
                                _ => Err(CalcError::UnexpectedToken(token_list[close_paren].to_string(), ")")),
                            }
                        } else {
                            Err(CalcError::UnmatchedParenthesis)
                        }
                    }
                    Err(e) => Err(e),
                }
            }
            _ => Err(CalcError::UnexpectedToken(token_list[0].to_string(), "number"))
        }
    } else {
        Err(CalcError::UnexpectedEndOfInput)
    }
}

pub fn parse(tokens: &[Token]) -> Result<String, CalcError> {
    e_expr(tokens).map(|answer| answer.value.to_string())
}

fn eval(input: &str) -> Result<String, CalcError> {
    tokenize(input).and_then(|x| parse(&x))
}

pub fn calc(args: &[&str]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if !args.is_empty() {
        let result = eval(&args.join(""))?;
        writeln!(stdout, "{}", result).map_err(CalcError::IO)?;
    } else {
        let prompt = b"[]> ";
        loop {
            let _ = stdout.write(prompt).map_err(CalcError::IO)?;
            let mut input = String::new();
            io::stdin().read_line(&mut input).map_err(CalcError::IO)?;
            if input.is_empty() {
                break;
            } else {
                match input.trim() {
                    "" => (),
                    "exit" => break,
                    s => {
                        writeln!(stdout, "{}", eval(s)?).map_err(CalcError::IO)?;
                    },
                }
            }
        }
    }
    Ok(())
}
