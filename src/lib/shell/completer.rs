use super::{
    directory_stack::DirectoryStack,
    escape::{escape, unescape},
    variables::Variables,
};
use auto_enums::auto_enum;
use glob::{glob_with, MatchOptions};
use liner::Completer;
use smallvec::SmallVec;
use std::{iter, str};

/// Performs escaping to an inner `FilenameCompleter` to enable a handful of special cases
/// needed by the shell, such as expanding '~' to a home directory, or adding a backslash
/// when a special character is contained within an expanded filename.
pub(crate) struct IonFileCompleter {
    /// A pointer to the directory stack in the shell.
    dir_stack: *const DirectoryStack,
    /// A pointer to the variables map in the shell.
    vars: *const Variables,
    /// The directory the expansion takes place in
    path: String,
}

impl IonFileCompleter {
    pub(crate) fn new(
        path: Option<&str>,
        dir_stack: *const DirectoryStack,
        vars: *const Variables,
    ) -> IonFileCompleter {
        let mut path = path.unwrap_or("").to_string();
        if !path.is_empty() && !path.ends_with('/') {
            path.push('/');
        }
        IonFileCompleter { dir_stack, vars, path }
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
        // Dereferencing the raw pointers here should be entirely safe, theoretically,
        // because no changes will occur to either of the underlying references in the
        // duration between creation of the completers and execution of their
        // completions.
        if let Some(expanded) = unsafe { (*self.vars).tilde_expansion(start, &*self.dir_stack) } {
            // Now we obtain completions for the `expanded` form of the `start` value.
            let iterator = filename_completion(&expanded, &self.path);

            // We can do that by obtaining the index position where the tilde character
            // ends. We don't search with `~` because we also want to
            // handle other tilde variants.
            let t_index = start.find('/').unwrap_or(1);
            // `tilde` is the tilde pattern, and `search` is the pattern that follows.
            let (tilde, search) = start.split_at(t_index);

            if search.len() < 2 {
                // If the length of the search pattern is less than 2, the search pattern is
                // empty, and thus the completions actually contain files and directories in
                // the home directory.

                // The tilde pattern will actually be our `start` command in itself,
                // and the completed form will be all of the characters beyond the length of
                // the expanded form of the tilde pattern.
                iterator.map(|completion| [start, &completion[expanded.len()..]].concat()).collect()
            // To save processing time, we should get obtain the index position where our
            // search pattern begins, and re-use that index to slice the completions so
            // that we may re-add the tilde character with the completion that follows.
            } else if let Some(e_index) = expanded.rfind(search) {
                // And then we will need to take those completions and remove the expanded form
                // of the tilde pattern and replace it with that pattern yet again.
                iterator
                    .map(|completion| escape(&[tilde, &completion[e_index..]].concat()))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            filename_completion(&start, &self.path).collect()
        }
    }
}

#[auto_enum]
fn filename_completion<'a, 'b>(start: &'a str, path: &'a str) -> impl Iterator<Item = String> + 'a {
    let unescaped_start = unescape(start);

    let mut split_start = unescaped_start.split('/');
    let mut string: SmallVec<[u8; 128]> = SmallVec::with_capacity(128);

    // When 'start' is an absolute path, "/..." gets split to ["", "..."]
    // So we skip the first element and add "/" to the start of the string
    if unescaped_start.starts_with('/') {
        split_start.next();
        string.push(b'/');
    } else {
        string.extend_from_slice(path.as_bytes());
    }

    for element in split_start {
        string.extend_from_slice(element.as_bytes());
        if element != "." && element != ".." {
            string.push(b'*');
        }
        string.push(b'/');
    }

    string.pop(); // pop out the last '/' character
    let string = unsafe { &str::from_utf8_unchecked(&string) };

    let globs = glob_with(
        string,
        MatchOptions {
            case_sensitive:              true,
            require_literal_separator:   true,
            require_literal_leading_dot: false,
        },
    )
    .ok()
    .map(|completions| {
        completions.filter_map(Result::ok).filter_map(move |file| {
            let out = file.to_str()?.trim_start_matches(&path);
            let mut joined = String::with_capacity(out.len() + 3); // worst case senario
            if unescaped_start.starts_with("./") {
                joined.push_str("./");
            }
            joined.push_str(out);
            if file.is_dir() {
                joined.push('/');
            }
            Some(joined)
        })
    });

    #[auto_enum(Iterator)]
    match globs {
        Some(iter) => iter,
        None => iter::once(escape(start)),
    }
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

    #[test]
    fn filename_completion() {
        let completer = IonFileCompleter::new(None, &DirectoryStack::new(), &Variables::default());
        assert_eq!(completer.completions("testing"), vec!["testing/"]);
        assert_eq!(completer.completions("testing/file"), vec!["testing/file_with_text"]);

        assert_eq!(completer.completions("~"), vec!["~/"]);

        assert_eq!(completer.completions("tes/fil"), vec!["testing/file_with_text"]);
    }
}
