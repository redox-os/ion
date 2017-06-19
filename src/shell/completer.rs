use liner::{Completer, FilenameCompleter};
use super::directory_stack::DirectoryStack;
use super::variables::Variables;

/// Performs escaping to an inner `FilenameCompleter` to enable a handful of special cases
/// needed by the shell, such as expanding '~' to a home directory, or adding a backslash
/// when a special character is contained within an expanded filename.
pub struct IonFileCompleter {
    /// The completer that this completer is  handling.
    inner: FilenameCompleter,
    /// A pointer to the directory stack in the shell.
    dir_stack: *const DirectoryStack,
    /// A pointer to the variables map in the shell.
    vars: *const Variables,
}

impl IonFileCompleter {
    pub fn new (
        path: Option<&str>,
        dir_stack: *const DirectoryStack,
        vars: *const Variables
    ) -> IonFileCompleter {
        IonFileCompleter {
            inner: FilenameCompleter::new(path),
            dir_stack: dir_stack,
            vars: vars
        }
    }
}

impl Completer for IonFileCompleter {
    fn completions(&self, start: &str) -> Vec<String> {
        if start.starts_with('~') {
            if let Some(expanded) = unsafe{ (*self.vars).tilde_expansion(start, &*self.dir_stack) } {
                let t_index = start.find('/').unwrap_or(1);
                let (tilde, search) = start.split_at(t_index as usize);
                let iterator = self.inner.completions(&expanded);
                let mut iterator = iterator.iter();
                let mut completions = Vec::new();

                if search.len() <= 1 {
                    for completion in iterator {
                        completions.push([start, &completion[expanded.len()..]].concat());
                    }
                } else {
                    if let Some(completion) = iterator.next() {
                        if let Some(e_index) = completion.find(search) {
                            completions.push(escape(&[tilde, &completion[e_index..]].concat()));
                            for completion in iterator {
                                let expanded = &completion[e_index..];
                                completions.push(escape(&[tilde, expanded].concat()));
                            }
                        }
                    }
                }

                return completions
            }
        }

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
