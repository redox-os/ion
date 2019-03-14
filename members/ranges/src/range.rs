use super::Index;

/// A range of values in a vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Range {
    /// Starting index
    start: Index,
    /// Ending index
    end: Index,
    /// Is this range inclusive? If false, this object represents a half-open
    /// range of [start, end), otherwise [start, end]
    inclusive: bool,
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

    pub fn exclusive(start: Index, end: Index) -> Range { Range { start, end, inclusive: false } }

    pub fn inclusive(start: Index, end: Index) -> Range { Range { start, end, inclusive: true } }

    pub fn from(start: Index) -> Range { Range { start, end: Index::new(-1), inclusive: true } }

    pub fn to(end: Index) -> Range { Range { start: Index::new(0), end, inclusive: false } }
}
