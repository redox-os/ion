use super::{directory_stack::DirectoryStack, variables::Variables};
use glob::glob;
use liner::{Completer, CursorPosition, FilenameCompleter};
use smallvec::SmallVec;
use toml;
use std::{cmp, collections::HashMap, env, iter, fs::File, io::{BufReader, prelude::*}, str};

pub(crate) struct IonCmdCompleter {
    ///
    completions: *const HashMap<String, CmdCompletion>,
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
        completions: *const HashMap<String, CmdCompletion>,
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

    fn file_completions(&self, start: &str) -> Vec<String> {
        let mut completions = Vec::new();
        //let file_completer = IonFileCompleter::new(start, self.dir_stack, self.vars);
        if let Ok(current_dir) = env::current_dir() {
            if let Some(url) = current_dir.to_str() {
                let completer =
                    IonFileCompleter::new(Some(url), self.dir_stack, self.vars);
                completions = completer.completions(start);
            }
        }

        completions
    }
}

impl Completer for IonCmdCompleter {
    /// When the tab key is pressed, **Liner** will use this method to perform completions of
    /// command parameters.
    fn completions(&self, start: &str) -> Vec<String> {
        //eprintln!("[cmd={}|start={}|pos={:?}]", self.cmd_so_far, start, self.pos);

        let file_completer = if let Ok(current_dir) = env::current_dir() {
            if let Some(url) = current_dir.to_str() {
                IonFileCompleter::new(Some(url), self.dir_stack, self.vars)
            } else {
                return Vec::new();
            }
        } else {
            return Vec::new();
        };

        // TODO: Should we respect IFS?
        let cmd = self.cmd_so_far.split_whitespace().collect::<Vec<&str>>();
        match unsafe { (*self.completions).get(cmd[0]) } {
            Some(completion) => {
                // get completions for application
                completion.get_completions(
                    cmd,
                    self.pos,
                    file_completer,
                )
            }
            None => {
                // return default (file) completions
                file_completer.completions(start)
            }
        }
    }
}

pub(crate) enum CmdParameter {
    FLAG,
    // bool tells us whether the parameter value is OPTIONAL
    PATH(bool),
    // if present, the vector gives a list of file extensions to match
    FILE(Option<Vec<String>>),
    // if present, the vector gives a list of possible key values
    KEYVALUE(Option<Vec<String>>),
    STRING
}

pub(crate) struct CmdCompletion {
    /// TODO
    name: String,
    params_short: HashMap<String, CmdParameter>,
    params_long: HashMap<String, CmdParameter>,
    subcommands: HashMap<String, CmdCompletion>
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

    pub(crate) fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_completions(&self, cmd: Vec<&str>, pos: CursorPosition, file_completer: IonFileCompleter) -> Vec<String> {
        let (idx, inword) = match pos {
            CursorPosition::InWord(idx) => (idx, true),
            CursorPosition::OnWordRightEdge(idx) => (idx, true),
            CursorPosition::OnWordLeftEdge(idx) => (idx-1, true),
            CursorPosition::InSpace(Some(idx), _) => (idx+1, false),
            CursorPosition::InSpace(_, _) => (1, false)
        };

        let completions = self.get_completions_int(cmd, idx, inword, file_completer);
        //println!("->completions: {:?}", completions);
        completions
    }

    fn get_completions_int(&self, cmd: Vec<&str>, idx: usize, inword: bool, file_completer: IonFileCompleter) -> Vec<String> {
        //println!("search in {} for {:?} with idx {} and inword: {}", self.name, cmd, idx, inword);
        if self.name != cmd[0] {
            return Vec::new()
        }

        let mut in_parameter = false;
        let mut parameter_type: Option<&CmdParameter> = None;
        let end = cmp::min(idx+1, cmd.len());

        //println!("idx: {}, len: {}, end: {}", idx, cmd.len(), end);
        for ii in 1..end {
            let item = cmd[ii];
            //println!("item({}): {}", ii, item);
            //println!("in_param: {}", in_parameter);

            if !in_parameter {
                // if we're in a submodule, pass the remaining parameters down
                if let Some(subcommand) = self.subcommands.get(item) {
                    let tmp = cmd[ii..cmd.len()].to_vec();
                    return subcommand.get_completions_int(tmp, idx-ii, inword, file_completer);
                }

                let lookuptable = if item.starts_with("--") {
                    Some(&self.params_long)
                } else if item.starts_with('-') {
                    Some(&self.params_short)
                } else {
                    None
                };

                if let Some(lookuptable) = lookuptable {
                    if let Some(param) = lookuptable.get(item) {
                        let (in_param, param_type) = match param {
                            CmdParameter::FLAG => (false, None),
                            param @ _ => (true, Some(param)),
                        };

                        in_parameter = in_param;
                        parameter_type = param_type;
                    }
                }
            // we have finished a parameter value only if:
            // - it is not at the end of the list
            // - it is the last item and we are not at the edge (but in a space behind)
            } else if ii != end - 1 || !inword {
                in_parameter = false;
                parameter_type = None;
            }
        }

        // At this point we know the type of the element we're typing right now:
        // - submodule name or parameter name
        // - parameter value (and which type it must be)
        let item_in_progress = if idx > 0 && idx < cmd.len() {
            cmd[idx]
        } else {
            ""
        };
        //println!("item_in_progress after loop: {} (inparam: {}) inword: {}", item_in_progress, in_parameter, inword);

        if !in_parameter {
            // submodule name or parameter name
            let candidates: Vec<String> = self.subcommands.keys()
                .chain(self.params_short.keys())
                .chain(self.params_long.keys())
                .filter(|xx| xx.starts_with(item_in_progress))
                .map(|xx| (*xx).clone() + " ")
                .collect();

            return if candidates.len() == 1 && candidates[0] == item_in_progress {
                // if only the element we just typed matches, check whether we already completed it
                if inword {
                    // if not, autocomplete with a space
                    vec!(item_in_progress.to_owned() + " ")
                } else {
                    // otherwise just add a space (this should never happen but we must return
                    // something so that the compiler is happy :P)
                    vec!(" ".to_owned())
                }
            } else {
                candidates
            }
        } else {
            // we are completing a parameter value
            if let Some(param_type) = parameter_type {
                match param_type {
                    CmdParameter::FILE(extensions) => {
                        let files = file_completer.completions(item_in_progress);
                        if let Some(extensions) = extensions {
                            let mut matching_files = Vec::new();
                            for extension in extensions {
                                matching_files = matching_files
                                    .iter()
                                    .chain(files.iter().filter(|xx| xx.ends_with(extension)))
                                    .map(|xx| (*xx).clone() + " ")
                                    .collect();
                            }

                            return matching_files;
                        } else {
                            return files;
                        }
                    },
                    CmdParameter::KEYVALUE(keys) => {
                        if let Some(keys) = keys {
                            return keys.iter()
                                .filter(|xx| xx.starts_with(item_in_progress))
                                .map(|xx| (*xx).clone() + "=")
                                .collect();
                        }
                    },
                    CmdParameter::PATH(optional) => {
                        // TODO: implement optional stuff
                        return file_completer.completions(item_in_progress);
                    },
                    CmdParameter::STRING => {
                        // no completions for that type available
                        return vec!();
                    },
                    _ => {
                        return vec!("NOT_YET_IMPLEMENTED".to_owned());
                    }
                }
            }
        }

        vec!(item_in_progress.to_owned() + " ")
    }

    fn from_internal(from: InternalCmdCompletion) -> CmdCompletion {
        let name = from.name;
        let (params_short, params_long) = if let Some(params) = from.params {
            let params_short = if let Some(params_short) = params.short {
                CmdCompletion::parse_internal_cmd_completion_param_set(params_short, "-")
            } else {
                HashMap::new()
            };

            let params_long = if let Some(params_long) = params.long {
                CmdCompletion::parse_internal_cmd_completion_param_set(params_long, "--")
            } else {
                HashMap::new()
            };

            (params_short, params_long)
        } else {
            (HashMap::new(), HashMap::new())
        };

        let mut subcommands: HashMap<String, CmdCompletion> = HashMap::new();
        if let Some(commands) = from.commands {
            for command in commands {
                let subcmd = CmdCompletion::from_internal(command);
                let subname = subcmd.get_name();
                subcommands.insert(subname, subcmd);
            }
        }

        CmdCompletion {
            name: name,
            params_short: params_short,
            params_long: params_long,
            subcommands: subcommands,
        }
    }

    fn parse_internal_cmd_completion_param_set(from: InternalCmdCompletionParamSet, prefix: &str) ->
    HashMap<String, CmdParameter> {
        let mut to = HashMap::new();

        if let Some(flags) = from.flag {
            for flag in &flags {
                to.insert(prefix.to_owned() + flag, CmdParameter::FLAG);
            }
        }

        if let Some(files) = from.file {
            for file in &files {
                let file_param = &file[0];
                let file_extensions = if file.len() > 1 {
                    Some(file[1..file.len()].to_vec())
                } else {
                    None
                };
                to.insert(prefix.to_owned() + file_param, CmdParameter::FILE(file_extensions));
            }
        }

        if let Some(paths) = from.path {
            for path in &paths {
                to.insert(prefix.to_owned() + path, CmdParameter::PATH(false));
            }
        }

        if let Some(paths) = from.path_optional {
            for path in &paths {
                to.insert(prefix.to_owned() + path, CmdParameter::PATH(true));
            }
        }

        if let Some(strings) = from.string {
            for string in &strings {
                to.insert(prefix.to_owned() + string, CmdParameter::STRING);
            }
        }

        if let Some(keyvalues) = from.keyvalue {
            for keyvalue in &keyvalues {
                let keyvalue_param = &keyvalue[0];
                let keyvalue_keys = if keyvalue.len() > 1 {
                    Some(keyvalue[1..keyvalue.len()].to_vec())
                } else {
                    None
                };
                to.insert(prefix.to_owned() + keyvalue_param, CmdParameter::KEYVALUE(keyvalue_keys));
            }
        }

        to
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
    file: Option<Vec<Vec<String>>>,
    path: Option<Vec<String>>,
    path_optional: Option<Vec<String>>,
    string: Option<Vec<String>>,
    keyvalue: Option<Vec<Vec<String>>>,
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
