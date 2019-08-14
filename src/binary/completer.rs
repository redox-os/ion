use auto_enums::auto_enum;
use glob::{glob_with, MatchOptions};
use ion_shell::{expansion::Expander, Shell};
use rustyline::{
    completion::{Completer, FilenameCompleter, Pair},
    highlight::Highlighter,
    hint::{Hinter, HistoryHinter},
    Context, Helper,
};
use std::{borrow::Cow, cell::RefCell, env, iter, num::NonZeroU8, path::PathBuf, str};

/// Unescape filenames for the completer so that special characters will be properly shown.
fn unescape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    let mut check = false;
    for character in input.bytes() {
        match character {
            b'\\' if !check => check = true,
            b'(' | b')' | b'[' | b']' | b'&' | b'$' | b'@' | b'{' | b'}' | b'<' | b'>' | b';'
            | b'"' | b'\'' | b'#' | b'^' | b'*' | b' '
                if check =>
            {
                output.push(character);
                check = false;
            }
            _ if check => {
                output.extend(&[b'\\', character]);
                check = false;
            }
            _ => output.push(character),
        }
    }
    unsafe { String::from_utf8_unchecked(output) }
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
            | b'"' | b'\'' | b'#' | b'^' | b'*' | b' ' => output.push(b'\\'),
            _ => (),
        }
        output.push(character);
    }
    unsafe { String::from_utf8_unchecked(output) }
}

fn getword(input: &str, pos: usize) -> (&str, usize, CompletionType) {
    if input.is_empty() {
        return (input, pos, CompletionType::Nothing);
    }

    let mut last_space = 0;
    let mut next_is_command = true;
    for word in input.split_ascii_whitespace() {
        if last_space + word.len() >= pos {
            let completion_type = if next_is_command {
                CompletionType::Command
            } else {
                CompletionType::VariableAndFiles
            };
            return (&input[last_space..last_space + word.len()], last_space, completion_type);
        } else {
            next_is_command = word.ends_with('|') || word.ends_with('&') || word.ends_with(';');
            last_space += word.len() + 1;
        }
    }
    let completion_type =
        if next_is_command { CompletionType::Command } else { CompletionType::VariableAndFiles };
    // This is the last word, so return the end of string
    (&input[last_space..], last_space, completion_type)
}

enum CompletionType {
    Nothing,
    Command,
    VariableAndFiles,
}

pub struct IonCompleter<'cell, 'builtins> {
    shell:         &'cell RefCell<Shell<'builtins>>,
    hinter:        HistoryHinter,
    count:         u8,
    load_on_flush: bool,
    save_each:     Option<NonZeroU8>,
}

impl<'a, 'b> IonCompleter<'a, 'b> {
    pub fn new(shell: &'a RefCell<Shell<'b>>) -> Self {
        IonCompleter {
            shell,
            hinter: HistoryHinter {},
            count: 0,
            save_each: NonZeroU8::new(10),
            load_on_flush: true,
        }
    }

    pub fn shell(&self) -> &'a RefCell<Shell<'b>> { &self.shell }

    pub fn should_save(&mut self) -> bool {
        self.count += 1;
        if self.save_each.map_or(false, |save_each| u8::from(save_each) <= self.count) {
            self.count = 0;
            true
        } else {
            false
        }
    }

    pub fn set_load_on_flush(&mut self, load_on_flush: bool) { self.load_on_flush = load_on_flush; }

    pub fn load_on_flush(&self) -> bool { self.load_on_flush }

    pub fn set_save_each(&mut self, save_each: u8) { self.save_each = NonZeroU8::new(save_each); }
}

impl<'a, 'b> Hinter for IonCompleter<'a, 'b> {
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl<'a, 'b> Helper for IonCompleter<'a, 'b> {}

impl<'a, 'cell> Highlighter for IonCompleter<'a, 'cell> {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Borrowed(prompt)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned("\x1b[37m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> { Cow::Borrowed(line) }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool { false }
}

impl<'a, 'b> Completer for IonCompleter<'a, 'b> {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let (start, word_pos, completion_type) = getword(line, pos);
        let shell = &self.shell.borrow();
        let vars = shell.variables();
        match completion_type {
            // Initialize a new completer from the definitions collected.
            // Creates a list of definitions from the shell environment that
            // will be used in the creation of a custom completer.
            // Add the list of available variables to the completer's
            // definitions.
            //
            // TODO: We should make it free to do String->SmallString and mostly free to go back
            // (free if allocated)
            CompletionType::VariableAndFiles if start.starts_with('$') => Ok((
                word_pos,
                vars.string_vars()
                    .filter(|(s, _)| s.starts_with(&start[1..]))
                    .map(|(s, _)| format!("${}", &s))
                    .map(|s| Pair { display: s.clone(), replacement: s })
                    .collect(),
            )),
            CompletionType::VariableAndFiles if start.starts_with('@') => Ok((
                word_pos,
                vars.arrays()
                    .filter(|(s, _)| s.starts_with(&start[1..]))
                    .map(|(s, _)| format!("@{}", &s))
                    .map(|s| Pair { display: s.clone(), replacement: s })
                    .collect(),
            )),
            CompletionType::VariableAndFiles => FilenameCompleter::new().complete(line, pos, ctx),
            CompletionType::Command => {
                // Initialize a new completer from the definitions collected.
                // Creates a list of definitions from the shell environment that
                // will be used
                // in the creation of a custom completer.
                let mut suggestions = shell
                    .builtins()
                    .keys()
                    // Add built-in commands to the completer's definitions.
                    .map(ToString::to_string)
                    // Add the aliases to the completer's definitions.
                    .chain(vars.aliases().map(|(key, _)| key.to_string()))
                    // Add the list of available functions to the completer's
                    // definitions.
                    .chain(vars.functions().map(|(key, _)| key.to_string()))
                    .filter(|s| s.starts_with(start))
                    .map(|s| Pair { display: s.clone(), replacement: s })
                    .collect::<Vec<_>>();
                // Creates completers containing definitions from all directories
                // listed
                // in the environment's **$PATH** variable.
                if let Some(paths) = env::var_os("PATH") {
                    for path in env::split_paths(&paths) {
                        let path = if !path.to_string_lossy().ends_with('/') {
                            let mut oss = path.into_os_string();
                            oss.push("/");
                            oss.into()
                        } else {
                            path
                        };
                        suggestions
                            .extend(IonFileCompleter::new(Some(path), &shell).completions(start));
                    }
                } else {
                    suggestions.extend(
                        IonFileCompleter::new(Some("/bin/".into()), &shell).completions(start),
                    )
                }
                Ok((word_pos, suggestions))
            }
            CompletionType::Nothing => Ok((line.len(), Vec::new())),
        }
    }
}

#[derive(Debug)]
struct WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    iter:       I,
    count:      usize,
    word_start: Option<usize>,
}
impl<I> WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    #[inline]
    fn check_boundary(&mut self, c: char, index: usize, escaped: bool) -> Option<(usize, usize)> {
        if let Some(start) = self.word_start {
            if c == ' ' && !escaped {
                self.word_start = None;
                Some((start, index))
            } else {
                self.next()
            }
        } else {
            if c != ' ' {
                self.word_start = Some(index);
            }
            self.next()
        }
    }
}
impl<I> Iterator for WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        self.count += 1;
        match self.iter.next() {
            Some((i, '\\')) => {
                if let Some((_, cnext)) = self.iter.next() {
                    self.count += 1;
                    // We use `i` in order to include the backslash as part of the word
                    self.check_boundary(cnext, i, true)
                } else {
                    self.next()
                }
            }
            Some((i, c)) => self.check_boundary(c, i, false),
            None => {
                // When start has been set, that means we have encountered a full word.
                self.word_start.take().map(|start| (start, self.count - 1))
            }
        }
    }
}

fn word_divide(buf: &str) -> Vec<(usize, usize)> {
    // -> impl Iterator<Item = (usize, usize)> + 'a
    WordDivide { iter: buf.chars().enumerate(), count: 0, word_start: None }.collect() // TODO: return iterator directly :D
}

/// Performs escaping to an inner `FilenameCompleter` to enable a handful of special cases
/// needed by the shell, such as expanding '~' to a home directory, or adding a backslash
/// when a special character is contained within an expanded filename.
#[derive(Clone)]
pub struct IonFileCompleter<'a: 'b, 'b> {
    shell: &'b Shell<'a>,
    /// The directory the expansion takes place in
    path: PathBuf,
    for_command: bool,
}

impl<'a, 'b> IonFileCompleter<'a, 'b> {
    pub fn new(path: Option<PathBuf>, shell: &'b Shell<'a>) -> Self {
        // The only time a path is Some is when looking for a command not a directory
        // so save this fact to strip the paths when completing commands.
        let for_command = path.is_some();
        let path = path.unwrap_or_default();
        IonFileCompleter { shell, path, for_command }
    }

    /// When the tab key is pressed, **Liner** will use this method to perform completions of
    /// filenames. As our `IonFileCompleter` is a wrapper around **Liner**'s
    /// `FilenameCompleter`,
    /// the purpose of our custom `Completer` is to expand possible `~` characters in the
    /// `start`
    /// value that we receive from the prompt, grab completions from the inner
    /// `FilenameCompleter`,
    /// and then escape the resulting filenames, as well as remove the expanded form of the `~`
    /// character and re-add the `~` character in it's place.
    fn completions(&self, start: &str) -> Vec<Pair> {
        // Dereferencing the raw pointers here should be entirely safe, theoretically,
        // because no changes will occur to either of the underlying references in the
        // duration between creation of the completers and execution of their
        // completions.
        let expanded = match self.shell.tilde(start) {
            Ok(expanded) => expanded,
            Err(why) => {
                eprintln!("ion: failed to autocomplete: {}", why);
                return Vec::new();
            }
        };
        // Now we obtain completions for the `expanded` form of the `start` value.
        let completions = filename_completion(&expanded, &self.path);
        if expanded == start {
            return if self.for_command {
                completions
                    .map(|s| s.rsplit('/').next().map(|s| s.to_string()).unwrap_or(s))
                    .map(|s| Pair { display: s.clone(), replacement: s })
                    .collect()
            } else {
                completions.map(|s| Pair { display: s.clone(), replacement: s }).collect()
            };
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
            completions
                .map(|completion| [start, &completion[expanded.len()..]].concat())
                .map(|s| Pair { display: s.clone(), replacement: s })
                .collect()
        // To save processing time, we should get obtain the index position where our
        // search pattern begins, and re-use that index to slice the completions so
        // that we may re-add the tilde character with the completion that follows.
        } else if let Some(e_index) = expanded.rfind(search) {
            // And then we will need to take those completions and remove the expanded form
            // of the tilde pattern and replace it with that pattern yet again.
            completions
                .map(|completion| [tilde, &completion[e_index..]].concat())
                .map(|s| Pair { display: s.clone(), replacement: s })
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[auto_enum]
fn filename_completion<'a>(start: &'a str, path: &'a PathBuf) -> impl Iterator<Item = String> + 'a {
    let unescaped_start = unescape(start);

    let mut split_start = unescaped_start.split('/');
    let mut string = String::with_capacity(128);

    // When 'start' is an absolute path, "/..." gets split to ["", "..."]
    // So we skip the first element and add "/" to the start of the string
    if unescaped_start.starts_with('/') {
        split_start.next();
        string.push('/');
    } else {
        string.push_str(&path.to_string_lossy());
    }

    for element in split_start {
        string.push_str(element);
        if element != "." && element != ".." {
            string.push('*');
        }
        string.push('/');
    }

    string.pop(); // pop out the last '/' character
    if string.ends_with('.') {
        string.push('*')
    }
    let globs = glob_with(
        &string,
        MatchOptions {
            case_sensitive:              true,
            require_literal_separator:   true,
            require_literal_leading_dot: false,
        },
    )
    .ok()
    .map(|completions| {
        completions.filter_map(Result::ok).filter_map(move |file| {
            let out = file.to_str()?;
            let mut joined = String::with_capacity(out.len() + 3); // worst case senario
            if unescaped_start.starts_with("./") {
                joined.push_str("./");
            }
            joined.push_str(out);
            if file.is_dir() {
                joined.push('/');
            }
            Some(escape(&joined))
        })
    });

    #[auto_enum(Iterator)]
    match globs {
        Some(iter) => iter,
        None => iter::once(start.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_comp<'a, 'b>(completer: &IonFileCompleter<'a, 'b>, comp: &str, results: &[&str]) {
        let comp = completer.completions(comp);
        assert!(comp.iter().map(|s| s.display.as_str()).eq(results.into_iter().map(|s| *s)));
    }

    #[test]
    fn filename_completion() {
        let shell = Shell::default();
        let completer = IonFileCompleter::new(None, &shell);
        assert_comp(&completer, "testing", &["testing/"]);
        assert_comp(&completer, "testing/file", &["testing/file_with_text"]);
        if cfg!(not(target_os = "redox")) {
            assert_comp(&completer, "~", &["~/"]);
        }
        assert_comp(&completer, "tes/fil", &["testing/file_with_text"]);
    }
}
