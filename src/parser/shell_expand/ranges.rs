use super::words::{Range, Index};

// SteppedRange is used because as of right now (start..end).step_by(step)
// is experimental/unstable. It is simply an iterator that creates a range
// (either backwards or forwards) with a given step which can be positive
// or negative. Since there are combinations of step sizes and start/end values
// that can cause infinite loops it comes with a `validate` method and a
// `new_validated` constructor. If the stepper isn't validated it will validate
// on first `next` call and silently return None if it is invalid
struct SteppedRange {
    start: isize,
    end: isize,
    step: isize,
    inclusive: bool,
    backwards: bool,
    validated: bool,
}

impl SteppedRange {
    #[inline]
    fn new(start: isize, end: isize, step:isize, inclusive: bool) -> Self {
        Self {
            validated: false,
            start: start,
            end: end,
            step: step,
            inclusive: inclusive,
            backwards: start > end,
        }
    }

    #[inline]
    fn new_validated(start: isize, end: isize, step:isize, inclusive: bool) -> Result<Self, &'static str> {
        let mut s = Self::new(start, end, step, inclusive);
        match s.validate() {
            Ok(_) => Ok(s),
            Err(why) => Err(why),            
        }
    }

     // Check for cases that would cause infinite loops 
    fn validate(&mut self) -> Result<(), &'static str> {
        if self.start < self.end && self.step < 0 {
            Err("negative step size with start < end would cause infinite loop")
        } else if self.start > self.end && self.step > 0 {
            Err("positive step size with start > end would cause infinite loop")
        } else if self.step == 0 {
            Err("0 step size would cause infinite loop")
        } else {
            self.validated = true;
            Ok(())
        }
    }

    // since we typically want to return a vec of strings instead of isize
    #[inline]
    fn new_string_vec(start: isize, end: isize, step:isize, inclusive: bool) -> Option<Vec<String>> {
        match Self::new_validated(start, end, step, inclusive) {
            Ok(s) => Some(s.map(|x| x.to_string()).collect()),
            Err(_) => None,
        }
    }
}

impl Iterator for SteppedRange {
    type Item = isize;
    fn next(&mut self) -> Option<Self::Item> {
        macro_rules! step_logic {
            ($left:expr, $right:expr) => {
                if self.inclusive && $left <= $right {
                    let v = self.start;
                    self.start += self.step;
                    Some(v)
                } else if !self.inclusive && $left < $right {
                    let v = self.start;
                    self.start += self.step;
                    Some(v)
                } else {
                    None
                }
            }
        }
        if !self.validated {
            match self.validate() {
                Ok(_) => self.validated = true,
                Err(_) => return None,
            }
        }
        // we always return and step `self.start`, but if it is backwards we
        // need to compare the other way around
        if self.backwards {
            step_logic!(self.end, self.start)
        } else {
            step_logic!(self.start, self.end)
        }
    }
}

pub fn parse_range(input: &str) -> Option<Vec<String>> {
    let mut bytes_iterator = input.bytes().enumerate();
    let mut parsed_first = false;
    let mut first = "";
    let mut step = 1;
    let mut dots = 0;
    while let Some((idx, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' | b'-' | b'a'...b'z' | b'A'...b'Z' => continue,
            b',' => {
                first = &input[..idx];
                parsed_first = true;
                // match what is following the , as the step size
                while let Some((inner_idx, inner_byte)) = bytes_iterator.next() {
                    match inner_byte {
                        b'0'...b'9' | b'-' => { continue },
                        b'.' => {
                            dots += 1;
                            step = match input[idx + 1..inner_idx].parse::<isize>() {
                                Ok(v) => v,
                                Err(_) => return None,
                            };
                            break;
                        },
                        _ => return None,
                    } 
                }
            }
            b'.' => {
                if !parsed_first {
                    first = &input[..idx];
                }
                dots += 1;
                while let Some((_, byte)) = bytes_iterator.next() {
                    if byte == b'.' { dots += 1 } else { break }
                }
            
                // 2 dots is exclusive 3 dots is inclusive
                // 1..3 -> 1 2
                // 1...3 -> 1 2 3
                if dots != 2 && dots != 3 { return None; }
                let inclusive = dots == 3;
            
                // when using the stepped range we already consumed one b'.' so the
                // index is off by 1
                let end = if parsed_first {
                    &input[idx+dots-1..]
                } else {
                    &input[idx+dots..]
                };
            
                if let Ok(start) = first.parse::<isize>() {
                    if let Ok(mut end) = end.parse::<isize>() {
                        return if step != 1 {
                            SteppedRange::new_string_vec(start, end, step, inclusive)
                        } else if start < end {
                            if inclusive {
                                end += 1;
                            }
                            Some((start..end).map(|x| x.to_string()).collect())
                        } else if start > end {
                            if dots == 2 {
                                end += 1;
                            }
                            Some((end..start+1).rev().map(|x| x.to_string()).collect())
                        } else {
                            Some(vec![first.to_owned()])
                        }
                    }
                } else if first.len() == 1 && end.len() == 1 {
                    let start = first.bytes().next().unwrap();
                    let mut end = end.bytes().next().unwrap();
            
                    let is_valid = ((start >= b'a' && start <= b'z') && (end >= b'a' && end <= b'z'))
                     || ((start >= b'A' && start <= b'Z') && (end >= b'A' && end <= b'Z'));
            
                    if !is_valid { break }
                    return if start < end {
                        if dots == 3 {
                            end += 1;
                        }
                        Some((start..end).map(|x| {
                            let mut output = String::with_capacity(1);
                            output.push(x as char);
                            output
                        }).collect())
                    } else if start > end {
                        if dots == 2 {
                            end += 1;
                        }
                        Some((end..start + 1).rev().map(|x| {
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

pub fn parse_index_range(input: &str) -> Option<Range> {
    let mut bytes_iterator = input.bytes().enumerate();
    while let Some((id, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' | b'-' => continue,
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
                            Ok(end) => Some(Range::to(Index::new(end))),
                            Err(_)  => None
                        }
                    }
                } else if end.is_empty() {
                    return match first.parse::<isize>() {
                        Ok(start) => Some(Range::from(Index::new(start))),
                        Err(_)    => None
                    }
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
    let valid_cases = vec![
        (Range::exclusive(Index::Forward(0), Index::Forward(3)), "0..3"),
        (Range::inclusive(Index::Forward(0), Index::Forward(2)), "0...2"),
        (Range::inclusive(Index::Forward(2), Index::Backward(1)), "2...-2"),
        (Range::inclusive(Index::Forward(0), Index::Backward(0)), "0...-1"),
        (Range::exclusive(Index::Backward(2), Index::Backward(0)), "-3..-1"),
        (Range::from(Index::Backward(2)), "-3.."),
        (Range::to(Index::Forward(5)), "..5")
    ];
    
    for (range, string) in valid_cases {
        assert_eq!(Some(range), parse_index_range(string));
    }

    let invalid_cases = vec![
        "0..A",
        "3-3..42"
    ];

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
    let expected = Some(vec![
        "a".to_owned(),
        "b".to_owned(),
        "c".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("c...a");
    let expected = Some(vec![
        "c".to_owned(),
        "b".to_owned(),
        "a".to_owned()
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("A...C");
    let expected = Some(vec![
        "A".to_owned(),
        "B".to_owned(),
        "C".to_owned(),
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("C...A");
    let expected = Some(vec![
        "C".to_owned(),
        "B".to_owned(),
        "A".to_owned()
    ]);

    assert_eq!(actual, expected);

    let actual = parse_range("C..A");
    let expected = Some(vec![
        "C".to_owned(),
        "B".to_owned(),
    ]);
    assert_eq!(actual, expected);

    let actual = parse_range("c..a");
    let expected = Some(vec![
        "c".to_owned(),
        "b".to_owned(),
    ]);
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
}
