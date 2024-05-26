use std::fmt::Display;

use super::Index;

/// A range of values in a vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Range {
    /// Starting index
    start:     Index,
    /// Ending index
    end:       Index,
    /// Interval to step by
    step:      Option<Index>,
    /// Is this range inclusive? If false, this object represents a half-open
    /// range of [start, end), otherwise [start, end]
    inclusive: bool,
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{})", self.start, self.end)
    }
}

impl Range {
    /// Returns the bounds of this range as a tuple containing:
    /// - The starting point of the range
    /// - The length of the range
    /// ```ignore,rust
    /// let vec = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
    /// let range = Range::exclusive(Index::new(1), Index::new(5));
    /// let (start, size) = range.bounds(vec.len()).unwrap();
    /// let expected = vec![1, 2, 3, 4];
    /// let selection = vec.iter().skip(start).take(size).collect::<Vec<_>>();
    /// assert_eq!(expected, selection);
    /// ```
    pub fn bounds(&self, vector_length: usize) -> Option<(usize, usize)> {
        let start = self.start.resolve(vector_length)?;
        let end = self.end.resolve(vector_length)?;
        if end < start {
            None
        } else if self.inclusive {
            Some((start, end - start + 1))
        } else {
            Some((start, end - start))
        }
    }

    pub fn exclusive(start: Index, end: Index, step: Option<Index>) -> Range {
        Range { start, end, inclusive: false, step }
    }

    pub fn inclusive(start: Index, end: Index, step: Option<Index>) -> Range {
        Range { start, end, inclusive: true, step }
    }

    pub fn from(start: Index, step: Option<Index>) -> Range {
        Range { start, end: Index::new(-1), inclusive: true, step }
    }

    pub fn to(end: Index, step: Option<Index>) -> Range {
        Range { start: Index::new(0), end, inclusive: false, step }
    }
}

impl<'a> Range {
    pub fn iter_array<T: std::fmt::Display + 'a>(
        &'a self,
        array_len: usize,
        array_iter: &'a mut (impl std::iter::DoubleEndedIterator<Item = T> + 'a),
    ) -> Option<impl std::iter::Iterator<Item = T> + 'a> {
        let modified_iter: Box<dyn std::iter::Iterator<Item = T>> = match self.step {
            Some(Index::Forward(0)) => return None,
            Some(Index::Forward(s)) => Box::new(std::iter::Iterator::step_by(array_iter, s)),
            Some(Index::Backward(s)) => Box::new(array_iter.rev().step_by(s + 1)),
            None => Box::new(array_iter),
        };
        self.bounds(array_len).and_then(|(start, length)| {
            if array_len > start {
                Some(modified_iter.skip(start).take(length))
            } else {
                None
            }
        })
    }
}
