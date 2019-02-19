use super::super::{completer::*, Binary, DirectoryStack, Shell, Variables};
use crate::{sys, types};
use liner::{BasicCompleter, CursorPosition, Event, EventKind};
use std::{env, io::ErrorKind, mem, path::PathBuf};

pub(crate) fn readln(shell: &mut Shell) -> Option<String> {
    {
        let vars_ptr = &shell.variables as *const Variables;
        let dirs_ptr = &shell.directory_stack as *const DirectoryStack;

        // Collects the current list of values from history for completion.
        let history = shell
            .context
            .as_ref()
            .unwrap()
            .history
            .buffers
            .iter()
            // Map each underlying `liner::Buffer` into a `String`.
            .map(|x| x.chars().cloned().collect())
            // Collect each result into a vector to avoid borrowing issues.
            .collect::<Vec<types::Str>>();

        {
            let prompt = shell.prompt();
            let vars = &shell.variables;
            let builtins = &shell.builtins;

            let line = shell.context.as_mut().unwrap().read_line(
                prompt,
                None,
                &mut move |Event { editor, kind }| {
                    if let EventKind::BeforeComplete = kind {
                        let (words, pos) = editor.get_words_and_cursor_position();

                        let filename = match pos {
                            CursorPosition::InWord(index) => index > 0,
                            CursorPosition::InSpace(Some(_), _) => true,
                            CursorPosition::InSpace(None, _) => false,
                            CursorPosition::OnWordLeftEdge(index) => index >= 1,
                            CursorPosition::OnWordRightEdge(index) => {
                                match (words.into_iter().nth(index), env::current_dir()) {
                                    (Some((start, end)), Ok(file)) => {
                                        let filename = editor.current_buffer().range(start, end);
                                        complete_as_file(&file, &filename, index)
                                    }
                                    _ => false,
                                }
                            }
                        };

                        if filename {
                            if let Ok(current_dir) = env::current_dir() {
                                if let Some(url) = current_dir.to_str() {
                                    let completer =
                                        IonFileCompleter::new(Some(url), dirs_ptr, vars_ptr);
                                    mem::replace(
                                        &mut editor.context().completer,
                                        Some(Box::new(completer)),
                                    );
                                }
                            }
                        } else {
                            // Creates a list of definitions from the shell environment that
                            // will be used
                            // in the creation of a custom completer.
                            let words = builtins
                                .keys()
                                .iter()
                                // Add built-in commands to the completer's definitions.
                                .map(|&s| s.to_string())
                                // Add the history list to the completer's definitions.
                                .chain(history.iter().map(|s| s.to_string()))
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
                                .collect();

                            // Initialize a new completer from the definitions collected.
                            let custom_completer = BasicCompleter::new(words);

                            // Creates completers containing definitions from all directories
                            // listed
                            // in the environment's **$PATH** variable.
                            let mut file_completers = if let Ok(val) = env::var("PATH") {
                                val.split(sys::PATH_SEPARATOR)
                                    .map(|s| IonFileCompleter::new(Some(s), dirs_ptr, vars_ptr))
                                    .collect()
                            } else {
                                vec![IonFileCompleter::new(Some("/bin/"), dirs_ptr, vars_ptr)]
                            };

                            // Also add files/directories in the current directory to the
                            // completion list.
                            if let Ok(current_dir) = env::current_dir() {
                                if let Some(url) = current_dir.to_str() {
                                    file_completers.push(IonFileCompleter::new(
                                        Some(url),
                                        dirs_ptr,
                                        vars_ptr,
                                    ));
                                }
                            }

                            // Merge the collected definitions with the file path definitions.
                            let completer = MultiCompleter::new(file_completers, custom_completer);

                            // Replace the shell's current completer with the newly-created
                            // completer.
                            mem::replace(
                                &mut editor.context().completer,
                                Some(Box::new(completer)),
                            );
                        }
                    }
                },
            );

            match line {
                Ok(line) => return Some(line),
                // Handles Ctrl + C
                Err(ref err) if err.kind() == ErrorKind::Interrupted => return None,
                // Handles Ctrl + D
                Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => (),
                Err(err) => {
                    eprintln!("ion: liner: {}", err);
                    return None;
                }
            }
        }
    }

    let previous_status = shell.previous_status;
    shell.exit(previous_status);
}

/// Infer if the given filename is actually a partial filename
fn complete_as_file(current_dir: &PathBuf, filename: &str, index: usize) -> bool {
    let filename = filename.trim();
    let mut file = current_dir.clone();
    file.push(&filename);
    // If the user explicitly requests a file through this syntax then complete as
    // a file
    if filename.starts_with('.') {
        return true;
    }
    // If the file starts with a dollar sign, it's a variable, not a file
    if filename.starts_with('$') {
        return false;
    }
    // Once we are beyond the first string, assume its a file
    if index > 0 {
        return true;
    }
    // If we are referencing a file that exists then just complete to that file
    if file.exists() {
        return true;
    }
    // If we have a partial file inside an existing directory, e.g. /foo/b when
    // /foo/bar exists, then treat it as file as long as `foo` isn't the
    // current directory, otherwise this would apply to any string `foo`
    if let Some(parent) = file.parent() {
        return parent.exists() && parent != current_dir;
    }
    // By default assume its not a file
    false
}
