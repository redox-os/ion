use super::{Index, Range};
use std::{cmp::Ordering, u8};

fn numeric_range<'a, K: From<String>>(
    start: isize,
    end: isize,
    step: isize,
    inclusive: bool,
    nb_digits: usize,
) -> Option<Box<dyn Iterator<Item = K> + 'a>> {
    let end = if start < end && inclusive {
        end + 1
    } else if start > end && inclusive {
        end - 1
    } else {
        end
    };

    if step == 0 || (start < end && step < 0) || (start > end && step > 0) {
        None
    } else {
        let (x, y, ordering) = if start < end {
            (start, end, Ordering::Greater)
        } else {
            (end, start, Ordering::Less)
        };

        let iter = (x..y).scan(start, move |index, _| {
            if end.cmp(index) == ordering {
                let index_holder = *index;
                *index += step; // This step adds
                Some(format!("{:0width$}", index_holder, width = nb_digits).into())
            } else {
                None
            }
        });

        Some(Box::new(iter))
    }
}

fn char_range<'a, K: From<String>>(
    start: u8,
    mut end: u8,
    step: isize,
    inclusive: bool,
) -> Option<Box<dyn Iterator<Item = K> + 'a>> {
    if !start.is_ascii_alphabetic() || !end.is_ascii_alphabetic() || step == 0 {
        return None;
    }

    if (start < end && inclusive) || (start > end && !inclusive) {
        end += 1;
    }

    let char_step = step.checked_abs()? as usize;
    if start < end {
        Some(Box::new((start..end).step_by(char_step).map(|x| (x as char).to_string().into())))
    } else {
        Some(Box::new(
            (end..=start).rev().step_by(char_step).map(|x| (x as char).to_string().into()),
        ))
    }
}

fn count_minimum_digits(a: &str) -> usize {
    match a.bytes().find(|&c| c != b'-') {
        Some(b'0') => a.len(),
        Some(b'1'..=b'9') => 0,
        Some(_) => panic!("count_minimum_digits should only be called for a valid number."),
        None => 0,
    }
}

fn finish<K: From<String>>(
    inclusive: bool,
    start_str: &str,
    end_str: &str,
    step: isize,
) -> Option<Box<dyn Iterator<Item = K>>> {
    if let (Ok(start), Ok(end)) = (start_str.parse::<isize>(), end_str.parse::<isize>()) {
        let step = if step == 1 && start >= end { -step } else { step };
        let nb_digits = usize::max(count_minimum_digits(start_str), count_minimum_digits(end_str));
        numeric_range(start, end, step, inclusive, nb_digits)
    } else if start_str.len() != 1 || end_str.len() != 1 {
        None
    } else {
        char_range(start_str.as_bytes()[0], end_str.as_bytes()[0], step, inclusive)
    }
}

// TODO: Make this an iterator structure.
// In a range we allow the following syntax:
//      Exclusive nonstepped: {start..end}
//      Inclusive nonstepped: {start...end}
//      Exclusive stepped: {start..step..end}
//      Inclusive stepped: {start..step...end}
pub fn parse_range<K: From<String>>(input: &str) -> Option<Box<dyn Iterator<Item = K>>> {
    let mut parts = input.split("..").collect::<Vec<_>>();
    let len = parts.len();

    // if the last separator contains three dots, this can only be an inclusive range
    let inclusive = parts.last()?.starts_with(|c| c == '.' || c == '=');
    if inclusive {
        parts[len - 1] = parts[len - 1].trim_start_matches(|c| c == '.' || c == '=');
    }

    match len {
        // two parts means unstepped range
        2 => finish(inclusive, parts[0], parts[1], 1),
        // middle string contains the step size
        3 => finish(inclusive, parts[0], parts[2], parts[1].parse::<isize>().ok()?),
        // not a valid byte for ranges
        _ => None,
    }
}

pub fn parse_index_range(input: &str) -> Option<Range> {
    let mut parts = input.splitn(3, "..");
    let range_to_use = RangeInput::new(parts);

    match range_to_use {
        // should this return all? have to fix how this works
        // RangeInput { start: None, end: None, step: None, .. } => {
        // Some(Range::inclusive(Index::new(0isize), Index::new(-1isize), None))
        // }

        // --== no steps ==--
        // range from
        RangeInput { start: Some(s), end: None, step: None, .. } => {
            Some(Range::from(Index::new(s), None))
        }
        // ranges to
        RangeInput { start: None, end: Some(e), step: None, inclusive: true } => {
            Some(Range::inclusive(Index::new(0), Index::new(e), None))
        }
        RangeInput { start: None, end: Some(e), step: None, inclusive: false } => {
            Some(Range::exclusive(Index::new(0), Index::new(e), None))
        }
        // complete ranges
        RangeInput { start: Some(s), end: Some(e), step: None, inclusive: true } => {
            Some(Range::inclusive(Index::new(s), Index::new(e), None))
        }
        RangeInput { start: Some(s), end: Some(e), step: None, inclusive: false } => {
            Some(Range::exclusive(Index::new(s), Index::new(e), None))
        }

        // --== steps ==--
        // range from
        RangeInput { start: Some(s), end: None, step: Some(step), .. } => {
            Some(Range::from(Index::new(s), Some(Index::new(step))))
        }
        // ranges to
        RangeInput { start: None, end: Some(e), step: Some(step), inclusive: true } => {
            Some(Range::inclusive(Index::new(0), Index::new(e), Some(Index::new(step))))
        }
        RangeInput { start: None, end: Some(e), step: Some(step), inclusive: false } => {
            Some(Range::exclusive(Index::new(0), Index::new(e), Some(Index::new(step))))
        }
        // complete ranges
        RangeInput { start: Some(s), end: Some(e), step: Some(step), inclusive: true } => {
            Some(Range::inclusive(Index::new(s), Index::new(e), Some(Index::new(step))))
        }
        RangeInput { start: Some(s), end: Some(e), step: Some(step), inclusive: false } => {
            Some(Range::exclusive(Index::new(s), Index::new(e), Some(Index::new(step))))
        }

        _ => None,
    }
}

#[derive(Debug)]
struct RangeInput {
    start:     Option<isize>,
    end:       Option<isize>,
    step:      Option<isize>,
    inclusive: bool,
}

impl<'a> RangeInput {
    fn new<T: std::iter::Iterator<Item = &'a str>>(mut parts_iter: T) -> RangeInput {
        let mut inclusive = false;
        let start = match parts_iter.next() {
            Some(s) => s.parse::<isize>().ok(),
            None => None,
        };
        let end = match parts_iter.next() {
            Some(e) => {
                inclusive = e.starts_with('.') || e.starts_with('=');
                if inclusive {
                    e[1..].parse::<isize>().ok()
                } else {
                    e.parse::<isize>().ok()
                }
            }
            None => None,
        };
        let step = match parts_iter.next() {
            Some(s) => s.parse::<isize>().ok(),
            None => None,
        };

        RangeInput { start, end, step, inclusive }
    }
}
