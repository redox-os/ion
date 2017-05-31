use liner::{Completer, FilenameCompleter};

pub struct IonFileCompleter {
    inner: FilenameCompleter
}

impl IonFileCompleter {
    pub fn new(path: Option<&str>) -> IonFileCompleter {
        IonFileCompleter { inner:  FilenameCompleter::new(path) }
    }
}

impl Completer for IonFileCompleter {
    fn completions(&self, start: &str) -> Vec<String> {
        self.inner.completions(start).iter().map(|x| escape(x.as_str())).collect()
    }
}

fn escape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    for character in input.bytes() {
        match character {
            b'(' | b')' | b'[' | b']' => output.push(b'\\'),
            _ => ()
        }
        output.push(character);
    }
    unsafe { String::from_utf8_unchecked(output) }
}

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
