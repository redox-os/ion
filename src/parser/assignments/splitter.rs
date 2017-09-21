/// Given an valid assignment expression, this will split it into `keys`, `operator`, `values`.
pub fn split_assignment<'a>(
    statement: &'a str,
) -> (Option<&'a str>, Option<&'a str>, Option<&'a str>) {
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
        assert_eq!(
            split_assignment("def ghi += 124 523"),
            (Some("def ghi"), Some("+="), Some("124 523"),)
        )
    }
}
