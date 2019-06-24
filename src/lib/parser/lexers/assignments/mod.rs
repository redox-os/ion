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
pub fn assignment_lexer(statement: &str) -> (Option<&str>, Option<Operator>, Option<&str>) {
    let statement = statement.trim();
    if statement.is_empty() {
        return (None, None, None);
    }

    let (mut read, mut start) = (0, 0);
    let as_bytes = statement.as_bytes();
    let mut bytes = statement.bytes().peekable();
    let mut operator = None;
    let mut delimiter_stack = Vec::new();

    while let Some(byte) = bytes.next() {
        operator = Some(Operator::Equal);

        if is_open_delimiter(byte) {
            delimiter_stack.push(byte);
        } else if delimiter_stack.last().map_or(false, |open| delimiters_match(*open, byte)) {
            delimiter_stack.pop();
        } else if delimiter_stack.is_empty() {
            if b'=' == byte {
                if bytes.peek().is_none() {
                    return (Some(&statement[..read].trim()), Some(Operator::Equal), None);
                }
                start = read;
                read += 1;
                break;
            }

            if let Some((op, found)) = find_operator(as_bytes, read) {
                operator = Some(op);
                start = read;
                read = found;
                break;
            }
        }

        read += 1;
    }

    if statement.len() == read {
        return (Some(statement.trim()), None, None);
    }

    let keys = statement[..start].trim_end();

    let values = &statement[read..];
    (Some(keys), operator, Some(values.trim()))
}

fn find_operator(bytes: &[u8], read: usize) -> Option<(Operator, usize)> {
    if bytes.len() < read + 3 {
        None
    } else if bytes[read + 1] == b'=' {
        Operator::parse_single(bytes[read]).map(|op| (op, read + 2))
    } else if bytes[read + 2] == b'=' {
        Operator::parse_double(&bytes[read..=read + 1]).map(|op| (op, read + 3))
    } else {
        None
    }
}

fn is_open_delimiter(byte: u8) -> bool { byte == b'[' }

fn delimiters_match(open: u8, close: u8) -> bool {
    match (open, close) {
        (b'[', b']') => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_splitting() {
        assert_eq!(assignment_lexer(""), (None, None, None));
        assert_eq!(assignment_lexer("abc"), (Some("abc"), None, None));

        assert_eq!(assignment_lexer("abc+=def"), (Some("abc"), Some(Operator::Add), Some("def")));

        assert_eq!(assignment_lexer("a+=b"), (Some("a"), Some(Operator::Add), Some("b")));

        assert_eq!(assignment_lexer("a=b"), (Some("a"), Some(Operator::Equal), Some("b")));

        assert_eq!(assignment_lexer("abc ="), (Some("abc"), Some(Operator::Equal), None));

        assert_eq!(assignment_lexer("abc =  "), (Some("abc"), Some(Operator::Equal), None));

        assert_eq!(
            assignment_lexer("abc = def"),
            (Some("abc"), Some(Operator::Equal), Some("def"))
        );

        assert_eq!(assignment_lexer("abc=def"), (Some("abc"), Some(Operator::Equal), Some("def")));

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
            (Some("abc def"), Some(Operator::OptionalEqual), Some("123 456"))
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

        assert_eq!(assignment_lexer("abc += def"), (Some("abc"), Some(Operator::Add), Some("def")));

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

    #[test]
    fn map_assignment() {
        assert_eq!(assignment_lexer("abc[=]"), (Some("abc[=]"), None, None));

        assert_eq!(
            assignment_lexer("abc['='] = '='"),
            (Some("abc['=']"), Some(Operator::Equal), Some("'='"))
        );

        assert_eq!(
            assignment_lexer("abc[=] = []=[]"),
            (Some("abc[=]"), Some(Operator::Equal), Some("[]=[]"))
        );
    }
}
