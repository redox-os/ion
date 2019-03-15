use super::{Index, Range};
use small;
use std::{cmp::Ordering, u8};

fn numeric_range<'a>(
    start: isize,
    end: isize,
    step: isize,
    inclusive: bool,
    nb_digits: usize,
) -> Option<Box<Iterator<Item = small::String> + 'a>> {
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

#[inline]
fn byte_is_valid_range(b: u8) -> bool { (b >= b'a' && b <= b'z') || (b >= b'A' && b <= b'Z') }

fn char_range<'a>(
    start: u8,
    end: u8,
    step: isize,
    inclusive: bool,
) -> Option<Box<Iterator<Item = small::String> + 'a>> {
    let char_step = step.checked_abs()?;
    if !byte_is_valid_range(start)
        || !byte_is_valid_range(end)
        || step == 0
        || char_step > u8::MAX as isize
    {
        return None;
    }

    let end = if start < end && inclusive {
        end + 1
    } else if start > end && inclusive {
        end - 1
    } else {
        end
    };
    let (x, y, ordering) =
        if start < end { (start, end, Ordering::Greater) } else { (end, start, Ordering::Less) };

    let iter = (x..y).scan(start, move |index, _| {
        if end.cmp(index) == ordering {
            let index_holder = *index;
            *index = match ordering {
                Ordering::Greater => index.wrapping_add(char_step as u8),
                Ordering::Less => index.wrapping_sub(char_step as u8),
                _ => unreachable!(),
            };
            Some((index_holder as char).to_string().into())
        } else {
            None
        }
    });

    Some(Box::new(iter))
}

fn count_minimum_digits(a: &str) -> usize {
    for c in a.chars() {
        match c {
            '-' => (),
            '0' => return a.len(),
            '1'...'9' => break,
            _ => panic!("count_minimum_digits should only be called for a valid number."),
        }
    }
    0
}

fn finish(inclusive: bool, start_str: &str, end_str: &str, step: isize) -> Option<Box<Iterator<Item = small::String>>> {
    if let (Ok(start), Ok(end)) = (start_str.parse::<isize>(), end_str.parse::<isize>())  {
        let step = if step == 1 {
            if start < end { step } else { -step }
        } else {
            step
        };
        let nb_digits = usize::max(count_minimum_digits(start_str), count_minimum_digits(end_str));
        numeric_range(start, end, step, inclusive, nb_digits)
    } else {
        char_range(start_str.bytes().next()?, end_str.bytes().next()?, step, inclusive)
    }
}

// TODO: Make this an iterator structure.
// In a range we allow the following syntax:
//      Exclusive nonstepped: {start..end}
//      Inclusive nonstepped: {start...end}
//      Exclusive stepped: {start..step..end}
//      Inclusive stepped: {start..step...end}
pub fn parse_range(input: &str) -> Option<Box<Iterator<Item = small::String>>> {
    let mut bytes_iterator = input.bytes().enumerate();
    let separator = bytes_iterator.by_ref().take_while(|&(i, b)| match b {
        b'a'...b'z' | b'-' | b'A'...b'Z' if i == 0 => true,
        b'0'...b'9' | b'.' => true,
        _ => false,
    }).position(|(_, b)| b == b'.')?;
    let start_str = &input[..separator];
    // The next byte has to be a dot to be valid range
    // syntax
    bytes_iterator.next().filter(|&(_, b)| b == b'.')?;

    // if the next byte is a dot we're certain it is an inclusive
    // unstepped range otherwise it has to be [-0-9a-zA-Z]
    bytes_iterator.next().and_then(|(start, b)| match b {
        b'.' | b'=' => {
            // this can only be an inclusive range
            finish(true, start_str, &input[start + 1..], 1)
        }
        b'0'...b'9' | b'-' | b'a'...b'z' | b'A'...b'Z' => {
            // further processing needed to find out if we're reading a step or
            // the end of an exclusive range. Step until we find another dot or
            // the iterator ends
            if let Some(end) = bytes_iterator.position(|(_, b)| b == b'.') {
                // stepped range input[start..read - 1] contains the step
                // size
                let step = (&input[start..=start + end]).parse::<isize>().ok()?;
                // count the dots to determine inclusive/exclusive
                let dots = bytes_iterator.take_while(|&(_, b)| b == b'.').count() + 1;
                finish(dots == 3, start_str, &input[start + end + dots + 1..], step)
            } else {
                // exhausted the iterator without finding anything new means
                // exclusive unstepped range
                finish(false, start_str, &input[start..], 1)
            }
        }
        // not a valid byte for ranges
        _ => None,
    })
}

pub fn parse_index_range(input: &str) -> Option<Range> {
    let mut bytes_iterator = input.bytes().enumerate();
    while let Some((id, byte)) = bytes_iterator.next() {
        match byte {
            b'0'...b'9' | b'-' => continue,
            b'.' => {
                let first = &input[..id];

                let mut dots = 1;
                let mut last_byte = 0;

                for (_, byte) in bytes_iterator {
                    last_byte = byte;
                    if byte == b'.' {
                        dots += 1
                    } else {
                        break;
                    }
                }

                if dots < 2 || dots > 3 {
                    break;
                }

                let inclusive = dots == 3 || (dots == 2 && last_byte == b'=');

                let end = &input[id + if inclusive { 3 } else { 2 }..];

                return if first.is_empty() && !end.is_empty() {
                    end.parse::<isize>().map(|end| Range::to(Index::new(end))).ok()
                } else if end.is_empty() {
                    first.parse::<isize>().map(|start| Range::from(Index::new(start))).ok()
                } else {
                    first
                        .parse::<isize>()
                        .and_then(|start| end.parse::<isize>().map(|end| (start, end)))
                        .map(|(start, end)| {
                            if inclusive {
                                Range::inclusive(Index::new(start), Index::new(end))
                            } else {
                                Range::exclusive(Index::new(start), Index::new(end))
                            }
                        })
                        .ok()
                };
            }
            _ => break,
        }
    }

    None
}
