use super::words::{Index, Range};

fn stepped_range_numeric<'a>(start: isize, end: isize, step: isize, nb_digits: usize) -> Option<Box<Iterator<Item = String> + 'a>> {
    return if step == 0 {
        None
    } else if start < end && step < 0 {
        None
    } else if start > end && step > 0 {
        None
    } else {
        if start < end {
            Some(Box::new((start..end).scan(start, move |index, _| {
                if *index < end {
                    let index_holder = *index;
                    *index += step; // This step adds
                    Some(format!("{:0width$}", index_holder, width=nb_digits))
                } else {
                    None
                }
            })))
        } else {
            Some(Box::new((end..start).scan(start, move |index, _| {
                if *index > end {
                    let index_holder = *index;
                    *index += step; // This step subtracts
                    Some(format!("{:0width$}", index_holder, width=nb_digits))
                } else {
                    None
                }
            })))
        }
    };
}

fn stepped_range_chars<'a>(start: u8, end: u8, step: u8) -> Option<Box<Iterator<Item = String> + 'a>> {
    if step == 0 {
        None
    } else {
        if start < end {
            Some(Box::new((start..end).scan(start, move |index, _| {
                if *index < end {
                    *index = index.wrapping_add(step);
                    Some((start as char).to_string())
                } else {
                    None
                }
            })))
        } else {
            Some(Box::new((end..start).scan(start, move |index, _| {
                if *index > end {
                    *index = index.wrapping_sub(step);
                    Some((start as char).to_string())
                } else {
                    None
                }
            })))
        }
    }
}

fn numeric_range<'a>(start: isize, mut end: isize, step: isize, inclusive: bool, nb_digits: usize)
    -> Option<Box<Iterator<Item = String> + 'a>> {
    if start < end {
        if inclusive {
            end += 1;
        }
        stepped_range_numeric(start, end, step, nb_digits)
    } else if start > end {
        if inclusive {
            end -= 1;
        }
        stepped_range_numeric(start, end, step, nb_digits)
    } else {
        Some(Box::new(Some(start.to_string()).into_iter()))
    }
}

#[inline]
fn byte_is_valid_range(b: u8) -> bool { (b >= b'a' && b <= b'z') || (b >= b'A' && b <= b'Z') }

use std::u8;
fn char_range<'a>(start: u8, mut end: u8, step: isize, inclusive: bool) -> Option<Box<Iterator<Item = String> + 'a>> {
    if !byte_is_valid_range(start) || !byte_is_valid_range(end) {
        return None;
    }

    let char_step = {
        let v = step.checked_abs()?;
        if v > u8::MAX as isize {
            return None;
        }
        v as u8
    };

    if start < end {
        if inclusive {
            end += 1;
        }
        stepped_range_chars(start, end, char_step)
    } else if start > end {
        if inclusive {
            end -= 1;
        }
        stepped_range_chars(start, end, char_step)
    } else {
        Some(Box::new(Some((start as char).to_string()).into_iter()))
    }
}

fn count_minimum_digits(a: &str) -> usize {
    let mut has_leading_zero = false;
    for c in a.chars() {
        match c {
            '-' => (),
            '0' => {
                has_leading_zero = true;
                break;
            },
            '1'...'9' => break,
            _ => panic!("count_minimum_digits should only be called for a valid number.")
        }
    }
    if !has_leading_zero { 0 }
    else { a.len() }
}

fn strings_to_isizes(a: &str, b: &str) -> Option<(isize, isize, usize)> {
    if let Ok(first) = a.parse::<isize>() {
        if let Ok(sec) = b.parse::<isize>() {
            let nb_digits = usize::max(count_minimum_digits(a), count_minimum_digits(b));
            Some((first, sec, nb_digits))
        } else {
            None
        }
    } else {
        None
    }
}

// In a range we allow the following syntax:
//      Exclusive nonstepped: {start..end}
//      Inclusive nonstepped: {start...end}
//      Exclusive stepped: {start..step..end}
//      Inclusive stepped: {start..step...end}
pub(crate) fn parse_range<'a>(input: &str) -> Option<Box<Iterator<Item = String> + 'a>> {
    let mut read = 0;
    let mut bytes_iterator = input.bytes();
    while let Some(byte) = bytes_iterator.next() {
        match byte {
            // can only find these as the first byte, otherwise the syntax is bad
            b'a'...b'z' | b'-' | b'A'...b'Z' if read == 0 => read += 1,
            b'0'...b'9' => read += 1,
            b'.' => {
                let first = &input[..read];
                read += 1;
                // The next byte has to be a dot to be valid range
                // syntax
                match bytes_iterator.next() {
                    Some(b'.') => read += 1,
                    _ => return None,
                }

                macro_rules! finish_char {
                    ($inclusive: expr, $end_str: expr, $step: expr) => {
                        if first.len() == 1 && $end_str.len() == 1 {
                            let start = first.as_bytes()[0];
                            let end = $end_str.as_bytes()[0];
                            return char_range(start, end, $step, $inclusive);
                        } else {
                            return None;
                        }
                    };
                }

                macro_rules! finish {
                    ($inclusive: expr, $read: expr) => {
                        let end_str = &input[$read..];
                        if let Some((start, end, nb_digits)) = strings_to_isizes(first, end_str) {
                            return numeric_range(
                                start,
                                end,
                                if start < end { 1 } else { -1 },
                                $inclusive,
                                nb_digits,
                            );
                        } else {
                            finish_char!($inclusive, end_str, 1);
                        }
                    };
                    ($inclusive: expr, $read: expr, $step: expr) => {
                        let end_str = &input[$read..];
                        if let Some((start, end, nb_digits)) = strings_to_isizes(first, end_str) {
                            return numeric_range(start, end, $step, $inclusive, nb_digits);
                        } else {
                            finish_char!($inclusive, end_str, $step);
                        }
                    };
                }

                // if the next byte is a dot we're certain it is an inclusive
                // unstepped range otherwise it has to be [-0-9a-zA-Z]
                if let Some(b) = bytes_iterator.next() {
                    read += 1;
                    match b {
                        b'.' => {
                            // this can only be an inclusive range
                            finish!(true, read);
                        }
                        b'0'...b'9' | b'-' | b'a'...b'z' | b'A'...b'Z' => {
                            // further processing needed to find out if we're reading a step or
                            // the end of an exclusive range. Step until we find another dot or
                            // the iterator ends
                            let start = read - 1;
                            while let Some(b) = bytes_iterator.next() {
                                read += 1;
                                match b {
                                    b'.' => {
                                        // stepped range input[start..read - 1] contains the step
                                        // size
                                        let step = match (&input[start..read - 1]).parse::<isize>()
                                        {
                                            Ok(v) => v,
                                            Err(_) => return None,
                                        };
                                        // count the dots to determine inclusive/exclusive
                                        let mut dots = 1;
                                        while let Some(b) = bytes_iterator.next() {
                                            read += 1;
                                            match b {
                                                b'.' => dots += 1,
                                                _ => break,
                                            }
                                        }
                                        finish!(dots == 3, read - 1, step);
                                    }
                                    // numeric values are OK but no letters anymore
                                    b'0'...b'9' => {}
                                    // unexpected
                                    _ => return None,
                                }
                            }
                            // exhausted the iterator without finding anything new means
                            // exclusive unstepped range
                            finish!(false, start);
                        }
                        // not a valid byte for ranges
                        _ => return None,
                    }
                }
            }
            _ => break,
        }
    }
    None
}

pub(crate) fn parse_index_range(input: &str) -> Option<Range> {
    let mut bytes_iterator = input.bytes().enumerate();
    while let Some((id, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' | b'-' => continue,
            b'.' => {
                let first = &input[..id];

                let mut dots = 1;
                while let Some((_, byte)) = bytes_iterator.next() {
                    if byte == b'.' {
                        dots += 1
                    } else {
                        break;
                    }
                }

                let inclusive = match dots {
                    2 => false,
                    3 => true,
                    _ => break,
                };

                let end = &input[id + dots..];

                if first.is_empty() {
                    return if end.is_empty() {
                        None
                    } else {
                        match end.parse::<isize>() {
                            Ok(end) => Some(Range::to(Index::new(end))),
                            Err(_) => None,
                        }
                    };
                } else if end.is_empty() {
                    return match first.parse::<isize>() {
                        Ok(start) => Some(Range::from(Index::new(start))),
                        Err(_) => None,
                    };
                }

                if let Ok(start) = first.parse::<isize>() {
                    if let Ok(end) = end.parse::<isize>() {
                        return Some(if inclusive {
                            Range::inclusive(Index::new(start), Index::new(end))
                        } else {
                            Range::exclusive(Index::new(start), Index::new(end))
                        });
                    }
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    None
}

#[test]
fn index_ranges() {
    let valid_cases = vec![
        (
            Range::exclusive(Index::Forward(0), Index::Forward(3)),
            "0..3",
        ),
        (
            Range::inclusive(Index::Forward(0), Index::Forward(2)),
            "0...2",
        ),
        (
            Range::inclusive(Index::Forward(2), Index::Backward(1)),
            "2...-2",
        ),
        (
            Range::inclusive(Index::Forward(0), Index::Backward(0)),
            "0...-1",
        ),
        (
            Range::exclusive(Index::Backward(2), Index::Backward(0)),
            "-3..-1",
        ),
        (Range::from(Index::Backward(2)), "-3.."),
        (Range::to(Index::Forward(5)), "..5"),
    ];

    for (range, string) in valid_cases {
        assert_eq!(Some(range), parse_index_range(string));
    }

    let invalid_cases = vec!["0..A", "3-3..42"];

    for range in invalid_cases {
        assert_eq!(None, parse_index_range(range))
    }
}

#[test]
fn range_expand() {
    assert_eq!(None, parse_range("abc"));

    let actual = parse_range("-3...3");
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

    let actual = parse_range("07...12");
    let expected = Some(vec![
        "07".to_owned(),
        "08".to_owned(),
        "09".to_owned(),
        "10".to_owned(),
        "11".to_owned(),
        "12".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("-3...10");
    let expected = Some(vec![
        "-3".to_owned(),
        "-2".to_owned(),
        "-1".to_owned(),
        "0".to_owned(),
        "1".to_owned(),
        "2".to_owned(),
        "3".to_owned(),
        "4".to_owned(),
        "5".to_owned(),
        "6".to_owned(),
        "7".to_owned(),
        "8".to_owned(),
        "9".to_owned(),
        "10".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("3...-3");
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

    let actual = parse_range("03...-3");
    let expected = Some(vec![
        "03".to_owned(),
        "02".to_owned(),
        "01".to_owned(),
        "00".to_owned(),
        "-1".to_owned(),
        "-2".to_owned(),
        "-3".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("3...-03");
    let expected = Some(vec![
        "003".to_owned(),
        "002".to_owned(),
        "001".to_owned(),
        "000".to_owned(),
        "-01".to_owned(),
        "-02".to_owned(),
        "-03".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("a...c");
    let expected = Some(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);

    assert_eq!(actual, expected);

    let actual = parse_range("c...a");
    let expected = Some(vec!["c".to_owned(), "b".to_owned(), "a".to_owned()]);

    assert_eq!(actual, expected);

    let actual = parse_range("A...C");
    let expected = Some(vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]);

    assert_eq!(actual, expected);

    let actual = parse_range("C...A");
    let expected = Some(vec!["C".to_owned(), "B".to_owned(), "A".to_owned()]);

    assert_eq!(actual, expected);

    let actual = parse_range("C..A");
    let expected = Some(vec!["C".to_owned(), "B".to_owned()]);
    assert_eq!(actual, expected);

    let actual = parse_range("c..a");
    let expected = Some(vec!["c".to_owned(), "b".to_owned()]);
    assert_eq!(actual, expected);

    let actual = parse_range("-3..4");
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

    let actual = parse_range("3..-4");
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

    let actual = parse_range("-3...0");
    let expected = Some(vec!["-3".into(), "-2".into(), "-1".into(), "0".into()]);
    assert_eq!(actual, expected);

    let actual = parse_range("-3..0");
    let expected = Some(vec!["-3".into(), "-2".into(), "-1".into()]);
    assert_eq!(actual, expected);
}
