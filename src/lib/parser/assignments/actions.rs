use super::{super::ArgumentSplitter, checker::*, *};
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq)]
pub(crate) enum AssignmentError<'a> {
    InvalidOperator(&'a str),
    InvalidValue(Primitive, Primitive),
    TypeError(TypeError<'a>),
}

impl<'a> Display for AssignmentError<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            AssignmentError::InvalidOperator(op) => write!(f, "invalid operator supplied: {}", op),
            AssignmentError::InvalidValue(expected, actual) => {
                write!(f, "expected {}, but received {}", expected, actual)
            }
            AssignmentError::TypeError(ref type_err) => write!(f, "{}", type_err),
        }
    }
}

/// An iterator structure which returns `Action` enums which tell the shell how to enact the
/// assignment request.
///
/// Each request will tell the shell whether the assignment is asking to update an array or a
/// string, and will contain the key/value pair to assign.
pub(crate) struct AssignmentActions<'a> {
    keys:     KeyIterator<'a>,
    operator: Operator,
    values:   ArgumentSplitter<'a>,
    prevkey:  &'a str,
    prevval:  &'a str,
}

impl<'a> AssignmentActions<'a> {
    pub(crate) fn new(keys: &'a str, operator: Operator, values: &'a str) -> AssignmentActions<'a> {
        AssignmentActions {
            keys: KeyIterator::new(keys),
            operator,
            values: ArgumentSplitter::new(values),
            prevkey: "",
            prevval: "",
        }
    }
}

impl<'a> Iterator for AssignmentActions<'a> {
    type Item = Result<Action<'a>, AssignmentError<'a>>;

    fn next(&mut self) -> Option<Result<Action<'a>, AssignmentError<'a>>> {
        if let Some(key) = self.keys.next() {
            match key {
                Ok(key) => match self.values.next() {
                    Some(value) => {
                        self.prevkey = key.name;
                        self.prevval = value;
                        Some(Action::new(key, self.operator, value))
                    }
                    None => None,
                },
                Err(why) => Some(Err(AssignmentError::TypeError(why))),
            }
        } else {
            if let Some(_) = self.values.next() {
                eprintln!(
                    "ion: extra values were supplied, and thus ignored. Previous assignment: '{}' \
                     = '{}'",
                    self.prevkey, self.prevval
                );
            }
            None
        }
    }
}

/// Defines which type of assignment action is to be performed.
///
/// Providing the key/value pair and operator to use during assignment, this variant defines
/// whether the assignment should set a string or array.
#[derive(Debug, PartialEq)]
pub(crate) enum Action<'a> {
    UpdateString(Key<'a>, Operator, &'a str),
    UpdateArray(Key<'a>, Operator, &'a str),
}

impl<'a> Action<'a> {
    fn new(
        var: Key<'a>,
        operator: Operator,
        value: &'a str,
    ) -> Result<Action<'a>, AssignmentError<'a>> {
        match var.kind {
            Primitive::AnyArray
            | Primitive::BooleanArray
            | Primitive::FloatArray
            | Primitive::IntegerArray
            | Primitive::StrArray => if is_array(value) {
                Ok(Action::UpdateArray(var, operator, value))
            } else {
                Err(AssignmentError::InvalidValue(var.kind, Primitive::Any))
            },
            Primitive::Any if is_array(value) => Ok(Action::UpdateArray(var, operator, value)),
            Primitive::Any => Ok(Action::UpdateString(var, operator, value)),
            _ if is_array(value) => {
                Err(AssignmentError::InvalidValue(var.kind, Primitive::AnyArray))
            }
            _ => Ok(Action::UpdateString(var, operator, value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(input: &str) -> (String, Operator, String) {
        let (keys, op, vals) = split_assignment(input);
        (
            keys.unwrap().into(),
            Operator::parse(op.unwrap()).unwrap(),
            vals.unwrap().into(),
        )
    }

    #[test]
    fn assignment_actions() {
        let (keys, op, vals) = split("abc def = 123 456");
        let actions = AssignmentActions::new(&keys, op, &vals).collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateString(
                Key {
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
                Key {
                    name: "def",
                    kind: Primitive::Any,
                },
                Operator::Equal,
                "456",
            ))
        );

        let (keys, op, vals) = split("ab:int *= 3");
        let actions = AssignmentActions::new(&keys, op, &vals).collect::<Vec<_>>();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateString(
                Key {
                    name: "ab",
                    kind: Primitive::Integer,
                },
                Operator::Multiply,
                "3",
            ))
        );

        let (keys, op, vals) = split("a b[] c:int[] = one [two three] [4 5 6]");
        let actions = AssignmentActions::new(&keys, op, &vals).collect::<Vec<_>>();
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateString(
                Key {
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
                Key {
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
                Key {
                    name: "c",
                    kind: Primitive::IntegerArray,
                },
                Operator::Equal,
                "[4 5 6]",
            ))
        );

        let (keys, op, values) = split("a[] b c[] = [one two] three [four five]");
        let actions = AssignmentActions::new(&keys, op, &values).collect::<Vec<_>>();
        assert_eq!(actions.len(), 3);
        assert_eq!(
            actions[0],
            Ok(Action::UpdateArray(
                Key {
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
                Key {
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
                Key {
                    name: "c",
                    kind: Primitive::AnyArray,
                },
                Operator::Equal,
                "[four five]",
            ))
        );
    }
}
