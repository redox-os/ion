mod keys;
mod operator;
mod primitive;

pub use self::{
    keys::{Key, KeyBuf, KeyIterator, TypeError},
    operator::Operator,
    primitive::Primitive,
};

/// Given an valid assignment expression, this will split it into `keys`,
/// `operator`, `values`.
pub fn assignment_lexer<'a>(
    statement: &'a str,
) -> (Option<&'a str>, Option<Operator>, Option<&'a str>) {
    let statement = statement.trim();
    if statement.is_empty() {
        return (None, None, None);
    }

    let (mut read, mut start) = (0, 0);
    let as_bytes = statement.as_bytes();
    let mut bytes = statement.bytes();
    let mut operator = None;

    while let Some(byte) = bytes.next() {
        if b'=' == byte {
            operator = Some(Operator::Equal);
            if as_bytes.get(read + 1).is_none() {
                return (Some(&statement[..read].trim()), operator, None);
            }
            start = read;
            read += 1;
            break;
        } else {
            match find_operator(as_bytes, read) {
                None => (),
                Some((op, found)) => {
                    operator = Some(op);
                    start = read;
                    read = found;
                    break;
                }
            }
        }
        read += 1;
    }

    if statement.len() == read {
        return (Some(statement.trim()), None, None);
    }

    let keys = statement[..start].trim_right();

    let values = &statement[read..];
    (Some(keys), operator, Some(values.trim()))
}

fn find_operator(bytes: &[u8], read: usize) -> Option<(Operator, usize)> {
    if bytes.len() < read + 3 {
        None
    } else if bytes[read + 1] == b'=' {
        Operator::parse_single(bytes[read]).map(|op| (op, read + 2))
    } else if bytes[read + 2] == b'=' {
        Operator::parse_double(&bytes[read..read + 2]).map(|op| (op, read + 3))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_splitting() {
        assert_eq!(assignment_lexer(""), (None, None, None));
        assert_eq!(assignment_lexer("abc"), (Some("abc"), None, None));

        assert_eq!(
            assignment_lexer("abc+=def"),
            (Some("abc"), Some(Operator::Add), Some("def"))
        );

        assert_eq!(
            assignment_lexer("a+=b"),
            (Some("a"), Some(Operator::Add), Some("b"))
        );

        assert_eq!(
            assignment_lexer("a=b"),
            (Some("a"), Some(Operator::Equal), Some("b"))
        );

        assert_eq!(
            assignment_lexer("abc ="),
            (Some("abc"), Some(Operator::Equal), None)
        );

        assert_eq!(
            assignment_lexer("abc =  "),
            (Some("abc"), Some(Operator::Equal), None)
        );

        assert_eq!(
            assignment_lexer("abc = def"),
            (Some("abc"), Some(Operator::Equal), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc=def"),
            (Some("abc"), Some(Operator::Equal), Some("def"))
        );

        assert_eq!(
            assignment_lexer("def ghi += 124 523"),
            (Some("def ghi"), Some(Operator::Add), Some("124 523"))
        )
    }

    #[test]
    fn assignment_assignments() {
        assert_eq!(
            assignment_lexer("a ?= b"),
            (Some("a"), Some(Operator::OptionalEqual), Some("b"))
        );

        assert_eq!(
            assignment_lexer("abc def ?= 123 456"),
            (
                Some("abc def"),
                Some(Operator::OptionalEqual),
                Some("123 456")
            )
        );
    }

    #[test]
    fn arithmetic_assignments() {
        assert_eq!(
            assignment_lexer("abc //= def"),
            (Some("abc"), Some(Operator::IntegerDivide), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc **= def"),
            (Some("abc"), Some(Operator::Exponent), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc += def"),
            (Some("abc"), Some(Operator::Add), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc -= def"),
            (Some("abc"), Some(Operator::Subtract), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc /= def"),
            (Some("abc"), Some(Operator::Divide), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc *= def"),
            (Some("abc"), Some(Operator::Multiply), Some("def"))
        );
    }

    #[test]
    fn concatenate_assignments() {
        assert_eq!(
            assignment_lexer("abc ++= def"),
            (Some("abc"), Some(Operator::Concatenate), Some("def"))
        );

        assert_eq!(
            assignment_lexer("abc::=def"),
            (Some("abc"), Some(Operator::ConcatenateHead), Some("def"))
        );
    }

    #[test]
    fn filter_assignment() {
        assert_eq!(
            assignment_lexer("abc \\\\= def"),
            (Some("abc"), Some(Operator::Filter), Some("def"))
        )
    }
}
