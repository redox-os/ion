use super::{directory_stack::DirectoryStack, variables::Variables};
use fnv::FnvHashMap;
use glob::glob;
use liner::{Completer, CursorPosition, FilenameCompleter};
//use serde_derive::Deserialize;
use smallvec::SmallVec;
use toml;
use std::{iter, str, fs::File, io::{BufReader, prelude::*}};

pub(crate) struct IonCmdCompleter {
    ///
    completions: *const FnvHashMap<String, CmdCompletion>,
    /// The entered command so far
    cmd_so_far: String,
    /// Where in the command we are
    pos: CursorPosition,
    /// A pointer to the directory stack in the shell.
    dir_stack: *const DirectoryStack,
    /// A pointer to the variables map in the shell.
    vars: *const Variables,
}

impl IonCmdCompleter {
    pub(crate) fn new(
        completions: *const FnvHashMap<String, CmdCompletion>,
        cmd_so_far: String,
        pos: CursorPosition,
        dir_stack: *const DirectoryStack,
        vars: *const Variables,
    ) -> IonCmdCompleter {
        IonCmdCompleter {
            completions: completions,
            cmd_so_far: cmd_so_far,
            pos: pos,
            dir_stack: dir_stack,
            vars: vars,
        }
    }
}

impl Completer for IonCmdCompleter {
    /// When the tab key is pressed, **Liner** will use this method to perform completions of
    /// command parameters.
    fn completions(&self, start: &str) -> Vec<String> {
        eprintln!("[cmd={}|start={}|pos={:?}]", self.cmd_so_far, start, self.pos);
        // TODO: Split at the right edge of the first word
        // let application = self.cmd_so_far.split
        //match self.completions.get(application) {
        //    Some(cmdCompletion) => // get completions for application
        //    None => // return default (file) completions
        //}
        Vec::new()
    }
}

pub(crate) enum CmdParameter {
    FLAG,
    PATH(bool),
    FILE,
    FILE_EXT(Vec<String>),
    DIRECTORY,
    KEYVALUE(Option<Vec<String>>),
    STRING
}

pub(crate) struct CmdCompletion {
    /// TODO
    name: String,
    params_short: FnvHashMap<String, CmdParameter>,
    params_long: FnvHashMap<String, CmdParameter>,
    subcommands: FnvHashMap<String, CmdCompletion>
}

impl CmdCompletion {
    pub(crate) fn from_config(configfile: String) -> Option<CmdCompletion> {
        let toml_str = if let Ok(file) = File::open(configfile) {
            let mut buf_reader = BufReader::new(file);
            let mut contents = String::new();
            if let Ok(_) = buf_reader.read_to_string(&mut contents){
                contents
            } else {
                "".to_owned()
            }
        } else {
            "".to_owned()
        };

        let decoded: Result<InternalCmdCompletion, _> = toml::from_str(&toml_str);
        if let Ok(decoded) = decoded {
            println!("{:#?}", decoded);
            Some(CmdCompletion::from_internal(decoded))
        } else {
            None
        }
    }

    fn from_internal(from: InternalCmdCompletion) -> CmdCompletion {
        let name = from.name;
        let (params_short, params_long) = if let Some(params) = from.params {
            let params_short = if let Some(params_short) = params.short {
                CmdCompletion::parse_InternalCmdCompletionParamSet(params_short)
            } else {
                FnvHashMap::default()
            };

            let params_long = if let Some(params_long) = params.long {
                CmdCompletion::parse_InternalCmdCompletionParamSet(params_long)
            } else {
                FnvHashMap::default()
            };

            (params_short, params_long)
        } else {
            (FnvHashMap::default(), FnvHashMap::default())
        };

        let mut subcommands: FnvHashMap<String, CmdCompletion> = FnvHashMap::default();
        if let Some(subcommands) = from.commands {
            for subcommand in subcommands {
                let subcmd = CmdCompletion::from_internal(subcommand);
                let subname = subcmd.name;
                // TODO: this does not work because FnvHashMap.insert() can only take a usize as key
                // Therefore, we need to use a std HashMap.
                //subcommands.insert(subname.as_bytes(), subcmd);
            }
        }

        CmdCompletion {
            name: name,
            params_short: params_short,
            params_long: params_long,
            subcommands: subcommands,
        }
    }

    fn parse_InternalCmdCompletionParamSet(from: InternalCmdCompletionParamSet) ->
    FnvHashMap<String, CmdParameter> {

        FnvHashMap::default()
    }
}

/// used to parse the config files, will then be translated to a CmdCompletion-struct
#[derive(Debug, Deserialize)]
struct InternalCmdCompletion {
    name: String,
    aliases: Option<Vec<String>>,
    end_of_params: Option<String>,
    after_params: Option<Vec<String>>,
    params: Option<InternalCmdCompletionParams>,
    commands: Option<Vec<InternalCmdCompletion>>,
}

#[derive(Debug, Deserialize)]
struct InternalCmdCompletionParams {
    short: Option<InternalCmdCompletionParamSet>,
    long: Option<InternalCmdCompletionParamSet>,
}

#[derive(Debug, Deserialize)]
struct InternalCmdCompletionParamSet {
    flag: Option<Vec<String>>,
    path: Option<Vec<String>>,
    path_optional: Option<Vec<String>>,
    string: Option<Vec<String>>,
    keyvalue: Option<Vec<String>>,
}

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
            inner: FilenameCompleter::new(path),
            dir_stack,
            vars,
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
                } else {
                    // To save processing time, we should get obtain the index position where our
                    // search pattern begins, and re-use that index to slice the completions so
                    // that we may re-add the tilde character with the completion that follows.
                    if let Some(completion) = iterator.next() {
                        if let Some(e_index) = expanded.rfind(search) {
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

        eprintln!("start={}", start);
        filename_completion(&start, |x| self.inner.completions(x)).collect()
    }
}

fn filename_completion<'a, LC>(start: &'a str, liner_complete: LC) -> impl Iterator<Item = String> + 'a
where
    LC: Fn(&str) -> Vec<String> + 'a
{
    let unescaped_start = unescape(start);

    let split_start = unescaped_start.split("/");
    let mut string: SmallVec<[u8; 128]> = SmallVec::with_capacity(128);

    // When 'start' is an absolute path, "/..." gets split to ["", "..."]
    // So we skip the first element and add "/" to the start of the string
    let skip = if unescaped_start.starts_with("/") {
        string.push(b'/');
        1
    } else {
        0
    };

    for element in split_start.skip(skip) {
        string.extend_from_slice(element.as_bytes());
        string.extend_from_slice(b"*/");
    }

    string.pop(); // pop out the last '/' character
    let string = unsafe { &str::from_utf8_unchecked(&string) };

    let globs = glob(string).ok().and_then(|completions| {
        let mut completions = completions
            .filter_map(Result::ok)
            .map(|x| x.to_string_lossy().into_owned());

        if let Some(first) = completions.next() {
            Some(iter::once(first).chain(completions))
        } else {
            None
        }
    });

    let iter_inner_glob: Box<Iterator<Item = String>> = match globs {
        Some(iter) => Box::new(iter),
        None => Box::new(iter::once(escape(start)))
    };

    // Use Liner::Completer as well, to preserve the previous behaviour
    // around single-directory completions
    iter_inner_glob.flat_map(move |path| {
        liner_complete(&path)
            .into_iter()
            .map(|x| escape(x.as_str()))
    })
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
        assert_eq!(
            completer.completions("testing/file"),
            vec!["testing/file_with_text"]
        );

        assert_eq!(completer.completions("~"), vec!["~/"]);

        assert_eq!(
            completer.completions("tes/fil"),
            vec!["testing/file_with_text"]
        );
    }
}
