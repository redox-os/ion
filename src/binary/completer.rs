use auto_enums::auto_enum;
use glob::{glob_with, MatchOptions};
use ion_shell::{
    expansion::{unescape, Expander},
    Shell,
};
use ion_sys::PATH_SEPARATOR;
use liner::{BasicCompleter, Completer, CursorPosition, Event, EventKind};
use smallvec::SmallVec;
use std::{env, iter, path::PathBuf, str};

pub struct IonCompleter<'a, 'b> {
    shell:             &'b Shell<'a>,
    history_completer: Option<BasicCompleter>,
}

/// Escapes filenames from the completer so that special characters will be properly escaped.
///
/// NOTE: Perhaps we should submit a PR to Liner to add a &'static [u8] field to
/// `FilenameCompleter` so that we don't have to perform the escaping ourselves?
fn escape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    for character in input.bytes() {
        match character {
            b'(' | b')' | b'[' | b']' | b'&' | b'$' | b'@' | b'{' | b'}' | b'<' | b'>' | b';'
            | b'"' | b'\'' | b'#' | b'^' | b'*' => output.push(b'\\'),
            _ => (),
        }
        output.push(character);
    }
    unsafe { String::from_utf8_unchecked(output) }
}

impl<'a, 'b> IonCompleter<'a, 'b> {
    pub fn new(shell: &'b Shell<'a>) -> Self { IonCompleter { shell, history_completer: None } }
}

impl<'a, 'b> Completer for IonCompleter<'a, 'b> {
    fn completions(&mut self, start: &str) -> Vec<String> {
        let mut completions = IonFileCompleter::new(None, &self.shell).completions(start);

        if let Some(ref mut history) = &mut self.history_completer {
            let vars = self.shell.variables();

            completions.extend(history.completions(start));
            // Initialize a new completer from the definitions collected.
            // Creates a list of definitions from the shell environment that
            // will be used
            // in the creation of a custom completer.
            completions.extend(
                self.shell
                    .builtins()
                    .keys()
                    // Add built-in commands to the completer's definitions.
                    .map(ToString::to_string)
                    // Add the aliases to the completer's definitions.
                    .chain(vars.aliases().map(|(key, _)| key.to_string()))
                    // Add the list of available functions to the completer's
                    // definitions.
                    .chain(vars.functions().map(|(key, _)| key.to_string()))
                    // Add the list of available variables to the completer's
                    // definitions. TODO: We should make
                    // it free to do String->SmallString
                    //       and mostly free to go back (free if allocated)
                    .chain(vars.string_vars().map(|(s, _)| ["$", &s].concat()))
                    .filter(|s| s.starts_with(start)),
            );
            // Creates completers containing definitions from all directories
            // listed
            // in the environment's **$PATH** variable.
            let file_completers: Vec<_> = env::var("PATH")
                .unwrap_or_else(|_| "/bin/".to_string())
                .split(PATH_SEPARATOR)
                .map(|s| IonFileCompleter::new(Some(s), &self.shell))
                .collect();
            // Merge the collected definitions with the file path definitions.
            completions.extend(MultiCompleter::new(file_completers).completions(start));
        }
        completions
    }

    fn on_event<W: std::io::Write>(&mut self, event: Event<'_, '_, W>) {
        if let EventKind::BeforeComplete = event.kind {
            let (words, pos) = event.editor.get_words_and_cursor_position();

            let filename = match pos {
                CursorPosition::InWord(index) => index > 0,
                CursorPosition::InSpace(Some(_), _) => true,
                CursorPosition::InSpace(None, _) => false,
                CursorPosition::OnWordLeftEdge(index) => index >= 1,
                CursorPosition::OnWordRightEdge(index) => words
                    .into_iter()
                    .nth(index)
                    .map(|(start, end)| event.editor.current_buffer().range(start, end))
                    .and_then(|filename| {
                        Some(complete_as_file(&env::current_dir().ok()?, &filename, index))
                    })
                    .filter(|&x| x)
                    .is_some(),
            };

            // Add the history list to the completer's definitions.
            // Map each underlying `liner::Buffer` into a `String`.
            self.history_completer = if filename {
                Some(BasicCompleter::new(
                    event
                        .editor
                        .context()
                        .history
                        .buffers
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                ))
            } else {
                None
            };
        }
    }
}

/// Performs escaping to an inner `FilenameCompleter` to enable a handful of special cases
/// needed by the shell, such as expanding '~' to a home directory, or adding a backslash
/// when a special character is contained within an expanded filename.
pub struct IonFileCompleter<'a, 'b> {
    shell: &'b Shell<'a>,
    /// The directory the expansion takes place in
    path: String,
}

impl<'a, 'b> IonFileCompleter<'a, 'b> {
    pub fn new(path: Option<&str>, shell: &'b Shell<'a>) -> Self {
        let mut path = path.unwrap_or("").to_string();
        if !path.is_empty() && !path.ends_with('/') {
            path.push('/');
        }
        IonFileCompleter { shell, path }
    }
}

impl<'a, 'b> Completer for IonFileCompleter<'a, 'b> {
    /// When the tab key is pressed, **Liner** will use this method to perform completions of
    /// filenames. As our `IonFileCompleter` is a wrapper around **Liner**'s
    /// `FilenameCompleter`,
    /// the purpose of our custom `Completer` is to expand possible `~` characters in the
    /// `start`
    /// value that we receive from the prompt, grab completions from the inner
    /// `FilenameCompleter`,
    /// and then escape the resulting filenames, as well as remove the expanded form of the `~`
    /// character and re-add the `~` character in it's place.
    fn completions(&mut self, start: &str) -> Vec<String> {
        // Dereferencing the raw pointers here should be entirely safe, theoretically,
        // because no changes will occur to either of the underlying references in the
        // duration between creation of the completers and execution of their
        // completions.
        let expanded = match self.shell.tilde(start) {
            Ok(expanded) => expanded,
            Err(why) => {
                eprintln!("ion: {}", why);
                return vec![start.into()];
            }
        };
        // Now we obtain completions for the `expanded` form of the `start` value.
        let completions = filename_completion(&expanded, &self.path);
        if expanded == start {
            return completions.collect();
        }
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
            completions.map(|completion| [start, &completion[expanded.len()..]].concat()).collect()
        // To save processing time, we should get obtain the index position where our
        // search pattern begins, and re-use that index to slice the completions so
        // that we may re-add the tilde character with the completion that follows.
        } else if let Some(e_index) = expanded.rfind(search) {
            // And then we will need to take those completions and remove the expanded form
            // of the tilde pattern and replace it with that pattern yet again.
            completions
                .map(|completion| escape(&[tilde, &completion[e_index..]].concat()))
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[auto_enum]
fn filename_completion<'a>(start: &'a str, path: &'a str) -> impl Iterator<Item = String> + 'a {
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
    if string.last() == Some(&b'.') {
        string.push(b'*')
    }
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
pub struct MultiCompleter<A>(Vec<A>);

impl<A> MultiCompleter<A> {
    pub fn new(completions: Vec<A>) -> Self { MultiCompleter(completions) }
}

impl<A> Completer for MultiCompleter<A>
where
    A: Completer,
{
    fn completions(&mut self, start: &str) -> Vec<String> {
        self.0.iter_mut().flat_map(|comp| comp.completions(start)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_completion() {
        let shell = Shell::library();
        let mut completer = IonFileCompleter::new(None, &shell);
        assert_eq!(completer.completions("testing"), vec!["testing/"]);
        assert_eq!(completer.completions("testing/file"), vec!["testing/file_with_text"]);
        assert_eq!(completer.completions("~"), vec!["~/"]);
        assert_eq!(completer.completions("tes/fil"), vec!["testing/file_with_text"]);
    }
}

/// Infer if the given filename is actually a partial filename
fn complete_as_file(current_dir: &PathBuf, filename: &str, index: usize) -> bool {
    let filename = filename.trim();
    let mut file = current_dir.clone();
    file.push(&filename);
    // If the user explicitly requests a file through this syntax then complete as
    // a file
    filename.starts_with('.') ||
    // If the file starts with a dollar sign, it's a variable, not a file
    (!filename.starts_with('$') &&
    // Once we are beyond the first string, assume its a file
    (index > 0 ||
    // If we are referencing a file that exists then just complete to that file
    file.exists() ||
    // If we have a partial file inside an existing directory, e.g. /foo/b when
    // /foo/bar exists, then treat it as file as long as `foo` isn't the
    // current directory, otherwise this would apply to any string `foo`
    file.parent().filter(|parent| parent.exists() && parent != current_dir).is_some()))
    // By default assume its not a file
}
