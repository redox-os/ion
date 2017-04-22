use liner::Completer;

/// A completer that combines suggestions from multiple completers.
#[derive(Clone, Eq, PartialEq)]
pub struct MultiCompleter<A, B> where A: Completer, B: Completer {
    a: Vec<A>,
    b: B
}

impl<A, B> MultiCompleter<A, B> where A: Completer, B: Completer {
    pub fn new(a: Vec<A>, b: B) -> MultiCompleter<A, B> {
        MultiCompleter {
            a: a,
            b: b
        }
    }
}

impl<A, B> Completer for MultiCompleter<A, B> where A: Completer, B: Completer {
    fn completions(&self, start: &str) -> Vec<String> {
        let mut completions = self.b.completions(start);
        for x in &self.a {
            completions.extend_from_slice(&x.completions(start));
        }
        completions
    }
}
