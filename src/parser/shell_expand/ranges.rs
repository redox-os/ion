use super::words::{Index, Range};

fn stepped_range_numeric(mut start: isize, end: isize, step: isize) -> Option<Vec<String>> {
    return if step == 0 {
        None
    } else if start < end && step < 0 {
        None
    } else if start > end && step > 0 {
        None
    } else {
        let mut out = Vec::new();
        let cmp: fn(isize, isize) -> bool = if start < end {
            |a: isize, b: isize| -> bool { a < b }
        } else {
            |a: isize, b: isize| -> bool { a > b }
        };
        while cmp(start, end) {
            out.push(start.to_string());
            start += step;
        }
        Some(out)
    }
}

fn stepped_range_chars(mut start: u8, end: u8, step: u8) -> Option<Vec<String>> {
    if step == 0 {
        None
    } else {
        let mut out = Vec::new();
        let cmp: fn(u8, u8) -> bool = if start < end {
            |a: u8, b: u8| -> bool { a < b }
        } else {
            |a: u8, b: u8| -> bool { a > b }
        };
        let step_func: fn(u8, u8) -> u8 = if start > end {
            |cur: u8, step: u8| -> u8 { cur.wrapping_sub(step) }
        } else {
            |cur: u8, step: u8| -> u8 { cur.wrapping_add(step) }
        };
        while cmp(start, end) {
            out.push((start as char).to_string());
            start = step_func(start, step);
        }
        Some(out)
    }
}

fn numeric_range(
    start: isize,
    mut end: isize,
    step: isize,
    inclusive: bool,
) -> Option<Vec<String>> {
    if start < end {
        if inclusive {
            end += 1;
        }
        stepped_range_numeric(start, end, step)
    } else if start > end {
        if inclusive {
            end += if end <= 0 { -1 } else { 1 };
        }
        stepped_range_numeric(start, end, step)
    } else {
        Some(vec![start.to_string()])
    }
}

#[inline]
fn byte_is_valid_range(b: u8) -> bool { (b >= b'a' && b <= b'z') || (b >= b'A' && b <= b'Z') }

use std::u8;
fn char_range(start: u8, mut end: u8, step: isize, inclusive: bool) -> Option<Vec<String>> {
    if !byte_is_valid_range(start) || !byte_is_valid_range(end) {
        return None
    }

    let char_step = match step.checked_abs() {
        Some(v) => if v > u8::MAX as isize {
            return None
        } else {
            v as u8
        },
        None => return None,
    };

    if start < end {
        if inclusive {
            end += 1;
        }
        return stepped_range_chars(start, end, char_step)
    } else if start > end {
        if inclusive {
            end -= 1;
        }
        return stepped_range_chars(start, end, char_step)
    } else {
        return Some(vec![(start as char).to_string()])
    }
}

fn strings_to_isizes(a: &str, b: &str) -> Option<(isize, isize)> {
    if let Ok(first) = a.parse::<isize>() {
        if let Ok(sec) = b.parse::<isize>() {
            Some((first, sec))
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
pub fn parse_range(input: &str) -> Option<Vec<String>> {
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
                    ($inclusive:expr, $end_str:expr, $step:expr) => {
                        if first.len() == 1 && $end_str.len() == 1 {
                            let start = first.as_bytes()[0];
                            let end = $end_str.as_bytes()[0];
                            return char_range(start, end, $step, $inclusive);
                        } else {
                            return None;
                        }
                    }
                }

                macro_rules! finish {
                    ($inclusive:expr, $read:expr) => {
                        let end_str = &input[$read..];
                        if let Some((start, end)) = strings_to_isizes(first, end_str) {
                            return numeric_range(start, end, if start < end { 1 } else { -1 }, $inclusive);
                        } else {
                            finish_char!($inclusive, end_str, 1);
                        }
                    };
                    ($inclusive:expr, $read:expr, $step:expr) => {
                        let end_str = &input[$read..];
                        if let Some((start, end)) = strings_to_isizes(first, end_str) {
                            return numeric_range(start, end, $step, $inclusive);
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
                                        // stepped range input[start..read - 1] contains the step size
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

pub fn parse_index_range(input: &str) -> Option<Range> {
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
                        break
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
                    }
                } else if end.is_empty() {
                    return match first.parse::<isize>() {
                        Ok(start) => Some(Range::from(Index::new(start))),
                        Err(_) => None,
                    }
                }

                if let Ok(start) = first.parse::<isize>() {
                    if let Ok(end) = end.parse::<isize>() {
                        return Some(if inclusive {
                            Range::inclusive(Index::new(start), Index::new(end))
                        } else {
                            Range::exclusive(Index::new(start), Index::new(end))
                        })
                    }
                } else {
                    break
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
        (Range::exclusive(Index::Forward(0), Index::Forward(3)), "0..3"),
        (Range::inclusive(Index::Forward(0), Index::Forward(2)), "0...2"),
        (Range::inclusive(Index::Forward(2), Index::Backward(1)), "2...-2"),
        (Range::inclusive(Index::Forward(0), Index::Backward(0)), "0...-1"),
        (Range::exclusive(Index::Backward(2), Index::Backward(0)), "-3..-1"),
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
