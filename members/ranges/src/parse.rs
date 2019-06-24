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
    let mut parts = input.splitn(2, "..");
    let first = parts.next()?;
    let mut end = parts.next()?;

    if first.is_empty() && !end.is_empty() {
        end.parse::<isize>().map(|end| Range::to(Index::new(end))).ok()
    } else if !end.is_empty() {
        let inclusive = end.starts_with('.') || end.starts_with('=');
        if inclusive {
            end = &end[1..];
        }

        let start = first.parse::<isize>().ok()?;
        let end = end.parse::<isize>().ok()?;
        if inclusive {
            Some(Range::inclusive(Index::new(start), Index::new(end)))
        } else {
            Some(Range::exclusive(Index::new(start), Index::new(end)))
        }
    } else {
        first.parse::<isize>().map(|start| Range::from(Index::new(start))).ok()
    }
}
