use liner::Completer;

/// A completer that combines suggestions from multiple completers.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct MultiCompleter<A, B> where A: Completer, B: Completer {
    a: A,
    b: B
}
impl<A, B> MultiCompleter<A, B> where A: Completer, B: Completer {
    pub fn new(a: A, b: B) -> MultiCompleter<A, B> {
        MultiCompleter {
            a: a,
            b: b
        }
    }
}
impl<A, B> Completer for MultiCompleter<A, B> where A: Completer, B: Completer {
    fn completions(&self, start: &str) -> Vec<String> {
        let mut completions = self.a.completions(start);
        completions.extend_from_slice(&self.b.completions(start));
        completions
    }
}