pub trait Batching<B, I, F>
    where I: Iterator,
          F: FnMut(&mut I) -> Option<B>,
{
    fn batching(self, func: F) -> BatchingIter<I, F>;
}

#[derive(Clone)]
pub struct BatchingIter<I, F> {
    iter: I,
    func: F,
}

impl<B, I, F> Batching<B, I, F> for I
    where I: Iterator,
          F: FnMut(&mut I) -> Option<B>,
{
    fn batching(self, func: F) -> BatchingIter<I, F> {
        BatchingIter { iter: self, func: func }
    }
}

impl<B, I, F> Iterator for BatchingIter<I, F>
    where I: Iterator,
          F: FnMut(&mut I) -> Option<B>,
{
    type Item = B;
    #[inline]
    fn next(&mut self) -> Option<B> {
        (self.func)(&mut self.iter)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
