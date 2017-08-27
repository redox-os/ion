use super::checker::*;
use super::parse::*;
use super::super::ArgumentSplitter;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operator {
    Add,
    Subtract,
    Divide,
    Multiply,
    Exponent,
    Equal,
}

impl Operator {
    fn parse<'a>(data: &'a str) -> Result<Operator, AssignmentError<'a>> {
        match data {
            "=" => Ok(Operator::Equal),
            "+=" => Ok(Operator::Add),
            "-=" => Ok(Operator::Subtract),
            "/=" => Ok(Operator::Divide),
            "*=" => Ok(Operator::Multiply),
            "**=" => Ok(Operator::Exponent),
            _ => Err(AssignmentError::InvalidOperator(data)),
        }
    }
}

impl Display for Operator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Operator::Add => write!(f, "+="),
            Operator::Subtract => write!(f, "-="),
            Operator::Divide => write!(f, "/="),
            Operator::Multiply => write!(f, "*="),
            Operator::Exponent => write!(f, "**="),
            Operator::Equal => write!(f, "="),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum AssignmentError<'a> {
    NoKeys,
    NoOperator,
    NoValues,
    InvalidOperator(&'a str),
    InvalidValue(Primitive, Primitive),
    TypeError(TypeError<'a>),
}

impl<'a> Display for AssignmentError<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            AssignmentError::NoKeys => write!(f, "no keys supplied"),
            AssignmentError::NoOperator => write!(f, "no operator supplied"),
            AssignmentError::NoValues => write!(f, "no values supplied"),
            AssignmentError::InvalidOperator(op) => write!(f, "invalid operator supplied: {}", op),
            AssignmentError::InvalidValue(expected, actual) => {
                write!(f, "expected {}, but received {}", expected, actual)
            }
            AssignmentError::TypeError(ref type_err) => write!(f, "{}", type_err),
        }
    }
}


pub struct AssignmentActions<'a> {
    keys: TypeParser<'a>,
    operator: Operator,
    values: ArgumentSplitter<'a>,
}

impl<'a> AssignmentActions<'a> {
    pub fn new(data: &'a str) -> Result<AssignmentActions<'a>, AssignmentError<'a>> {
        let (keys, op, vals) = split_assignment(data);
        Ok(AssignmentActions {
            keys: keys.map(TypeParser::new).ok_or(AssignmentError::NoKeys)?,
            operator: Operator::parse(op.ok_or(AssignmentError::NoOperator)?)?,
            values: vals.map(ArgumentSplitter::new).ok_or(
                AssignmentError::NoValues,
            )?,
        })
    }
}

impl<'a> Iterator for AssignmentActions<'a> {
    type Item = Result<Action<'a>, AssignmentError<'a>>;
    fn next(&mut self) -> Option<Result<Action<'a>, AssignmentError<'a>>> {
        if let Some(key) = self.keys.next() {
            match key {
                Ok(key) => {
                    match self.values.next() {
                        Some(value) => Some(Action::new(key, self.operator, value)),
                        None => None,
                    }
                }
                Err(why) => Some(Err(AssignmentError::TypeError(why))),
            }
        } else {
            if let Some(_) = self.values.next() {
                eprintln!("ion: extra values were supplied and ignored");
            }
            None
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Action<'a> {
    UpdateString(TypeArg<'a>, Operator, &'a str),
    UpdateArray(TypeArg<'a>, Operator, &'a str),
}

impl<'a> Action<'a> {
    fn new(var: TypeArg<'a>, operator: Operator, value: &'a str) -> Result<Action<'a>, AssignmentError<'a>> {
        match var.kind {
            Primitive::AnyArray | Primitive::BooleanArray | Primitive::FloatArray | Primitive::IntegerArray |
            Primitive::StrArray => {
                if is_array(value) {
                    Ok(Action::UpdateArray(var, operator, value))
                } else {
                    Err(AssignmentError::InvalidValue(var.kind, Primitive::Any))
                }
            }
            Primitive::Any if is_array(value) => Ok(Action::UpdateArray(var, operator, value)),
            Primitive::Any => Ok(Action::UpdateString(var, operator, value)),
            _ if is_array(value) => Err(AssignmentError::InvalidValue(var.kind, Primitive::AnyArray)),
            _ => Ok(Action::UpdateString(var, operator, value)),
        }
    }
}

/// Given an valid assignment expression, this will split it into `keys`, `operator`, `values`.
fn split_assignment<'a>(statement: &'a str) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
    let statement = statement.trim();
    if statement.len() == 0 {
        return (None, None, None);
    }

    let mut read = 0;
    let mut bytes = statement.bytes();
    let mut start = 0;

    while let Some(byte) = bytes.next() {
        if b'=' == byte {
            if let None = statement.as_bytes().get(read + 1) {
                return (Some(&statement[..read].trim()), Some("="), None);
            }
            start = read;
            read += 1;
            break;
        } else if [b'+', b'-', b'/', b'*'].contains(&byte) {
            start = read;
            read += 1;
            while let Some(byte) = bytes.next() {
                read += 1;
                if byte == b'=' {
                    break;
                }
            }
            break;
        }
        read += 1;
    }

    if statement.len() == read {
        return (Some(statement.trim()), None, None);
    }

    let keys = statement[..start].trim_right();

    let operator = &statement[start..read];
    if read == statement.len() {
        return (Some(keys), Some(operator), None);
    }

    let values = &statement[read..];
    (Some(keys), Some(operator), Some(values.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_splitting() {
        assert_eq!(split_assignment(""), (None, None, None));
        assert_eq!(split_assignment("abc"), (Some("abc"), None, None));
        assert_eq!(split_assignment("abc+=def"), (Some("abc"), Some("+="), Some("def")));
        assert_eq!(split_assignment("abc ="), (Some("abc"), Some("="), None));
        assert_eq!(split_assignment("abc =  "), (Some("abc"), Some("="), None));
        assert_eq!(split_assignment("abc = def"), (Some("abc"), Some("="), Some("def")));
        assert_eq!(split_assignment("abc=def"), (Some("abc"), Some("="), Some("def")));
        assert_eq!(split_assignment("def ghi += 124 523"), (
            Some("def ghi"),
            Some("+="),
            Some("124 523"),
        ))
    }

    #[test]
    fn assignment_actions() {
        let actions = AssignmentActions::new("abc def = 123 456")
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateString(
                TypeArg {
                    name: "abc",
                    kind: Primitive::Any,
                },
                Operator::Equal,
                "123",
            ))
        );
        assert_eq!(
            actions[1],
            Ok(Action::UpdateString(
                TypeArg {
                    name: "def",
                    kind: Primitive::Any,
                },
                Operator::Equal,
                "456",
            ))
        );

        let actions = AssignmentActions::new("ab:int *= 3")
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateString(
                TypeArg {
                    name: "ab",
                    kind: Primitive::Integer,
                },
                Operator::Multiply,
                "3",
            ))
        );

        let actions = AssignmentActions::new("a b[] c:int[] = one [two three] [4 5 6]")
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateString(
                TypeArg {
                    name: "a",
                    kind: Primitive::Any,
                },
                Operator::Equal,
                "one",
            ))
        );
        assert_eq!(
            actions[1],
            Ok(Action::UpdateArray(
                TypeArg {
                    name: "b",
                    kind: Primitive::AnyArray,
                },
                Operator::Equal,
                "[two three]",
            ))
        );
        assert_eq!(
            actions[2],
            Ok(Action::UpdateArray(
                TypeArg {
                    name: "c",
                    kind: Primitive::IntegerArray,
                },
                Operator::Equal,
                "[4 5 6]",
            ))
        );

        let actions = AssignmentActions::new("a[] b c[] = [one two] three [four five]")
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateArray(
                TypeArg {
                    name: "a",
                    kind: Primitive::AnyArray,
                },
                Operator::Equal,
                "[one two]",
            ))
        );
        assert_eq!(
            actions[1],
            Ok(Action::UpdateString(
                TypeArg {
                    name: "b",
                    kind: Primitive::Any,
                },
                Operator::Equal,
                "three",
            ))
        );
        assert_eq!(
            actions[2],
            Ok(Action::UpdateArray(
                TypeArg {
                    name: "c",
                    kind: Primitive::AnyArray,
                },
                Operator::Equal,
                "[four five]",
            ))
        );
    }
}
