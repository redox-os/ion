/// Index into a vector-like object
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Index {
    /// Index starting from the beginning of the vector, where `Forward(0)`
    /// is the first element
    Forward(usize),
    /// Index starting from the end of the vector, where `Backward(0)` is the
    /// last element. `
    Backward(usize),
}

impl Index {
    pub fn resolve(&self, vector_length: usize) -> Option<usize> {
        match *self {
            Index::Forward(n) => Some(n),
            Index::Backward(n) => if n >= vector_length {
                None
            } else {
                Some(vector_length - (n + 1))
            },
        }
    }

    /// Construct an index using the following convetions:
    /// - A positive value `n` represents `Forward(n)`
    /// - A negative value `-n` reprents `Backwards(n - 1)` such that:
    /// ```ignore,rust
    /// assert_eq!(Index::new(-1), Index::Backward(0))
    /// ```
    pub fn new(input: isize) -> Index {
        if input < 0 {
            Index::Backward((input.abs() as usize) - 1)
        } else {
            Index::Forward(input.abs() as usize)
        }
    }
}
