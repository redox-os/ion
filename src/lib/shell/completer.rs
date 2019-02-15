use super::{
    directory_stack::DirectoryStack,
    escape::{escape, unescape},
    variables::Variables,
};
use glob::glob;
use liner::{Completer, FilenameCompleter};
use smallvec::SmallVec;
use std::{iter, str};

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
        IonFileCompleter { inner: FilenameCompleter::new(path), dir_stack, vars }
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
            // duration between creation of the completers and execution of their
            // completions.
            if let Some(expanded) = unsafe { (*self.vars).tilde_expansion(start, &*self.dir_stack) }
            {
                // Now we obtain completions for the `expanded` form of the `start` value.
                let mut iterator = filename_completion(&expanded, |x| self.inner.completions(x));

                // And then we will need to take those completions and remove the expanded form
                // of the tilde pattern and replace it with that pattern yet again.
                let mut completions = Vec::new();

                // We can do that by obtaining the index position where the tilde character
                // ends. We don't search with `~` because we also want to
                // handle other tilde variants.
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
                // To save processing time, we should get obtain the index position where our
                // search pattern begins, and re-use that index to slice the completions so
                // that we may re-add the tilde character with the completion that follows.
                } else if let Some(completion) = iterator.next() {
                    if let Some(e_index) = expanded.rfind(search) {
                        completions.push(escape(&[tilde, &completion[e_index..]].concat()));
                        for completion in iterator {
                            let expanded = &completion[e_index..];
                            completions.push(escape(&[tilde, expanded].concat()));
                        }
                    }
                }

                return completions;
            }
        } else if start.starts_with("./") && unescape(start).split('/').count() == 2 {
            // Special case for ./scripts, the globbing code removes the ./
            return self.inner.completions(&start);
        }

        filename_completion(&start, |x| self.inner.completions(x)).collect()
    }
}

fn filename_completion<'a, LC>(
    start: &'a str,
    liner_complete: LC,
) -> impl Iterator<Item = String> + 'a
where
    LC: Fn(&str) -> Vec<String> + 'a,
{
    let unescaped_start = unescape(start);

    let split_start = unescaped_start.split('/');
    let mut string: SmallVec<[u8; 128]> = SmallVec::with_capacity(128);

    // When 'start' is an absolute path, "/..." gets split to ["", "..."]
    // So we skip the first element and add "/" to the start of the string
    let skip = if unescaped_start.starts_with('/') {
        string.push(b'/');
        1
    } else {
        0
    };

    for element in split_start.skip(skip) {
        if element != ".." && element != "." {
            string.extend_from_slice(element.as_bytes());
            string.extend_from_slice(b"*/");
        } else {
            string.extend_from_slice(element.as_bytes());
            string.push(b'/');
        }
    }

    string.pop(); // pop out the last '/' character
    let string = unsafe { &str::from_utf8_unchecked(&string) };

    let globs = glob(string).ok().and_then(|completions| {
        let mut completions =
            completions.filter_map(Result::ok).map(|x| x.to_string_lossy().into_owned()).peekable();

        if completions.peek().is_some() {
            Some(completions)
        } else {
            None
        }
    });

    let iter_inner_glob: Box<Iterator<Item = String>> = match globs {
        Some(iter) => Box::new(iter),
        None => Box::new(iter::once(escape(start))),
    };

    // Use Liner::Completer as well, to preserve the previous behaviour
    // around single-directory completions
    iter_inner_glob
        .flat_map(move |path| liner_complete(&path).into_iter().map(|x| escape(x.as_str())))
}

/// A completer that combines suggestions from multiple completers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MultiCompleter<A, B>
where
    A: Completer,
    B: Completer,
{
    a: Vec<A>,
    b: B,
}

impl<A, B> MultiCompleter<A, B>
where
    A: Completer,
    B: Completer,
{
    pub(crate) fn new(a: Vec<A>, b: B) -> MultiCompleter<A, B> { MultiCompleter { a, b } }
}

impl<A, B> Completer for MultiCompleter<A, B>
where
    A: Completer,
    B: Completer,
{
    fn completions(&self, start: &str) -> Vec<String> {
        let mut completions = self.b.completions(start);
        for x in &self.a {
            completions.extend_from_slice(&x.completions(start));
        }
        completions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn filename_completion() {
        let current_dir = env::current_dir().expect("Unable to get current directory");

        let completer = IonFileCompleter::new(
            current_dir.to_str(),
            &DirectoryStack::new(),
            &Variables::default(),
        );
        assert_eq!(completer.completions("testing"), vec!["testing/"]);
        assert_eq!(completer.completions("testing/file"), vec!["testing/file_with_text"]);

        assert_eq!(completer.completions("~"), vec!["~/"]);

        assert_eq!(completer.completions("tes/fil"), vec!["testing/file_with_text"]);
    }
}
