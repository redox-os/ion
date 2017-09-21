use super::directory_stack::DirectoryStack;
use super::variables::Variables;
use liner::{Completer, FilenameCompleter};

/// Performs escaping to an inner `FilenameCompleter` to enable a handful of special cases
/// needed by the shell, such as expanding '~' to a home directory, or adding a backslash
/// when a special character is contained within an expanded filename.
pub(crate) struct IonFileCompleter {
    /// The completer that this completer is  handling.
    inner: FilenameCompleter,
    /// A pointer to the directory stack in the shell.
    dir_stack: *const DirectoryStack,
    /// A pointer to the variables map in the shell.
    vars: *const Variables,
}

impl IonFileCompleter {
    pub(crate) fn new(
        path: Option<&str>,
        dir_stack: *const DirectoryStack,
        vars: *const Variables,
    ) -> IonFileCompleter {
        IonFileCompleter {
            inner:     FilenameCompleter::new(path),
            dir_stack: dir_stack,
            vars:      vars,
        }
    }
}

impl Completer for IonFileCompleter {
    /// When the tab key is pressed, **Liner** will use this method to perform completions of
    /// filenames. As our `IonFileCompleter` is a wrapper around **Liner**'s
    /// `FilenameCompleter`,
    /// the purpose of our custom `Completer` is to expand possible `~` characters in the
    /// `start`
    /// value that we receive from the prompt, grab completions from the inner
    /// `FilenameCompleter`,
    /// and then escape the resulting filenames, as well as remove the expanded form of the `~`
    /// character and re-add the `~` character in it's place.
    fn completions(&self, start: &str) -> Vec<String> {
        // Only if the first character is a tilde character will we perform expansions
        if start.starts_with('~') {
            // Dereferencing the raw pointers here should be entirely safe, theoretically,
            // because no changes will occur to either of the underlying references in the
            // duration between creation of the completers and execution of their completions.
            if let Some(expanded) = unsafe { (*self.vars).tilde_expansion(start, &*self.dir_stack) }
            {
                // Now we obtain completions for the `expanded` form of the `start` value.
                let completions = self.inner.completions(&expanded);
                let mut iterator = completions.iter();

                // And then we will need to take those completions and remove the expanded form
                // of the tilde pattern and replace it with that pattern yet again.
                let mut completions = Vec::new();

                // We can do that by obtaining the index position where the tilde character ends.
                // We don't search with `~` because we also want to handle other tilde variants.
                let t_index = start.find('/').unwrap_or(1);
                // `tilde` is the tilde pattern, and `search` is the pattern that follows.
                let (tilde, search) = start.split_at(t_index as usize);

                if search.len() < 2 {
                    // If the length of the search pattern is less than 2, the search pattern is
                    // empty, and thus the completions actually contain files and directories in
                    // the home directory.

                    // The tilde pattern will actually be our `start` command in itself,
                    // and the completed form will be all of the characters beyond the length of
                    // the expanded form of the tilde pattern.
                    for completion in iterator {
                        completions.push([start, &completion[expanded.len()..]].concat());
                    }
                } else {
                    // To save processing time, we should get obtain the index position where our
                    // search pattern begins, and re-use that index to slice the completions so
                    // that we may re-add the tilde character with the completion that follows.
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

                return completions;
            }
        }

        self.inner.completions(&unescape(start)).iter().map(|x| escape(x.as_str())).collect()
    }
}

/// Escapes filenames from the completer so that special characters will be properly escaped.
///
/// NOTE: Perhaps we should submit a PR to Liner to add a &'static [u8] field to
/// `FilenameCompleter` so that we don't have to perform the escaping ourselves?
fn escape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    for character in input.bytes() {
        match character {
            b'('
            | b')'
            | b'['
            | b']'
            | b'&'
            | b'$'
            | b'@'
            | b'{'
            | b'}'
            | b'<'
            | b'>'
            | b';'
            | b'"'
            | b'\'' => output.push(b'\\'),
            _ => (),
        }
        output.push(character);
    }
    unsafe { String::from_utf8_unchecked(output) }
}

/// Unescapes filenames to be passed into the completer
fn unescape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        match b {
            b'\\' => if let Some(next) = bytes.next() {
                output.push(next);
            } else {
                output.push(b'\\')
            },
            _ => output.push(b),
        }
    }
    unsafe { String::from_utf8_unchecked(output) }
}

/// A completer that combines suggestions from multiple completers.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct MultiCompleter<A, B>
    where A: Completer,
          B: Completer
{
    a: Vec<A>,
    b: B,
}

impl<A, B> MultiCompleter<A, B>
    where A: Completer,
          B: Completer
{
    pub(crate) fn new(a: Vec<A>, b: B) -> MultiCompleter<A, B> { MultiCompleter { a: a, b: b } }
}

impl<A, B> Completer for MultiCompleter<A, B>
    where A: Completer,
          B: Completer
{
    fn completions(&self, start: &str) -> Vec<String> {
        let mut completions = self.b.completions(start);
        for x in &self.a {
            completions.extend_from_slice(&x.completions(start));
        }
        completions
    }
}
