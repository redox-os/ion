use super::Index;

/// A range of values in a vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub(crate) struct Range {
    /// Starting index
    start: Index,
    /// Ending index
    end: Index,
    /// Is this range inclusive? If false, this object represents a half-open
    /// range of [start, end), otherwise [start, end]
    inclusive: bool,
}

impl Range {
    pub(crate) fn to(end: Index) -> Range {
        Range {
            start: Index::new(0),
            end,
            inclusive: false,
        }
    }

    pub(crate) fn from(start: Index) -> Range {
        Range {
            start,
            end: Index::new(-1),
            inclusive: true,
        }
    }

    pub(crate) fn inclusive(start: Index, end: Index) -> Range {
        Range {
            start,
            end,
            inclusive: true,
        }
    }

    pub(crate) fn exclusive(start: Index, end: Index) -> Range {
        Range {
            start,
            end,
            inclusive: false,
        }
    }

    /// Returns the bounds of this range as a tuple containing:
    /// - The starting point of the range
    /// - The length of the range
    /// ```
    /// let vec = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
    /// let range = Range::exclusive(Index::new(1), Index::new(5));
    /// let (start, size) = range.bounds(vec.len()).unwrap();
    /// let expected = vec![1, 2, 3, 4];
    /// let selection = vec.iter().skip(start).take(size).collect::<Vec<_>>();
    /// assert_eq!(expected, selection);
    /// ```
    pub(crate) fn bounds(&self, vector_length: usize) -> Option<(usize, usize)> {
        if let Some(start) = self.start.resolve(vector_length) {
            if let Some(end) = self.end.resolve(vector_length) {
                if end < start {
                    None
                } else if self.inclusive {
                    Some((start, end - start + 1))
                } else {
                    Some((start, end - start))
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}
