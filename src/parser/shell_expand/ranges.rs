use super::words::{IndexStart, IndexEnd};

pub fn parse_range(input: &str) -> Option<Vec<String>> {
    let mut bytes_iterator = input.bytes().enumerate();
    while let Some((id, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' | b'-' | b'a'...b'z' | b'A'...b'Z' => continue,
            b'.' => {
                let first = &input[..id];

                let mut dots = 1;
                while let Some((_, byte)) = bytes_iterator.next() {
                    if byte == b'.' { dots += 1 } else { break }
                }

                if dots != 2 { break }

                let end = &input[id+dots..];

                if let Ok(start) = first.parse::<isize>() {
                    if let Ok(end) = end.parse::<isize>() {
                        return if start < end {
                            Some((start..end+1).map(|x| x.to_string()).collect())
                        } else if start > end {
                            Some((end..start+1).rev().map(|x| x.to_string()).collect())
                        } else {
                            Some(vec![first.to_owned()])
                        }
                    }
                } else if first.len() == 1 && end.len() == 1 {
                    let start = first.bytes().next().unwrap();
                    let end = end.bytes().next().unwrap();

                    let is_valid = ((start >= b'a' && start <= b'z') && (end >= b'a' && end <= b'z'))
                     || ((start >= b'A' && start <= b'Z') && (end >= b'A' && end <= b'Z'));

                    if !is_valid { break }
                    return if start < end {
                        Some((start..end+1).map(|x| {
                            let mut output = String::with_capacity(1);
                            output.push(x as char);
                            output
                        }).collect())
                    } else if start > end {
                        Some((end..start+1).rev().map(|x| {
                            let mut output = String::with_capacity(1);
                            output.push(x as char);
                            output
                        }).collect())
                    } else {
                        Some(vec![first.to_owned()])
                    }
                } else {
                    break
                }
            },
            _ => break
        }
    }

    None
}

pub fn parse_index_range(input: &str) -> Option<(IndexStart, IndexEnd)> {
    let mut bytes_iterator = input.bytes().enumerate();
    while let Some((id, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' => continue,
            b'.' => {
                let first = &input[..id];

                let mut dots = 1;
                while let Some((_, byte)) = bytes_iterator.next() {
                    if byte == b'.' { dots += 1 } else { break }
                }

                let inclusive = match dots {
                    2 => false,
                    3 => true,
                    _ => break
                };

                let end = &input[id+dots..];

                if first.is_empty() {
                    return if end.is_empty() {
                        None
                    } else {
                        match end.parse::<isize>() {
                            Ok(end) => Some((IndexStart::new(0), IndexEnd::new(end))),
                            Err(_)  => None
                        }
                    }
                } else if end.is_empty() {
                    return match first.parse::<isize>() {
                        Ok(start) => Some((IndexStart::new(start), IndexEnd::CatchAll)),
                        Err(_)    => None
                    }
                }

                if let Ok(start) = first.parse::<isize>() {
                    if let Ok(end) = end.parse::<isize>() {
                        return if inclusive {
                            Some((IndexStart::new(start), IndexEnd::FromEnd((end+1) as usize)))
                        } else {
                            Some((IndexStart::new(start), IndexEnd::new(end)))
                        }
                    }
                } else {
                    break
                }
            },
            _ => break
        }
    }

    None
}


#[test]
fn index_ranges() {
    assert_eq!(Some((IndexStart::new(0), IndexEnd::new(3))), parse_index_range("0..3"));
    assert_eq!(Some((IndexStart::new(0), IndexEnd::FromEnd(3))), parse_index_range("0...2"));
    assert_eq!(None, parse_index_range("0..A"));
}

#[test]
fn range_expand() {
    assert_eq!(None, parse_range("abc"));

    let actual = parse_range("-3..3");
    let expected = Some(vec![
        "-3".to_owned(),
        "-2".to_owned(),
        "-1".to_owned(),
        "0".to_owned(),
        "1".to_owned(),
        "2".to_owned(),
        "3".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("3..-3");
    let expected = Some(vec![
        "3".to_owned(),
        "2".to_owned(),
        "1".to_owned(),
        "0".to_owned(),
        "-1".to_owned(),
        "-2".to_owned(),
        "-3".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("a..c");
    let expected = Some(vec![
        "a".to_owned(),
        "b".to_owned(),
        "c".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("c..a");
    let expected = Some(vec![
        "c".to_owned(),
        "b".to_owned(),
        "a".to_owned()
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("A..C");
    let expected = Some(vec![
        "A".to_owned(),
        "B".to_owned(),
        "C".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("C..A");
    let expected = Some(vec![
        "C".to_owned(),
        "B".to_owned(),
        "A".to_owned()
    ]);

    assert_eq!(actual, expected);
}
