use super::{super::completer::*, InteractiveBinary};
use crate::{sys, types};
use liner::{BasicCompleter, CursorPosition, Event, EventKind};
use std::{env, io::ErrorKind, mem, path::PathBuf};

pub(crate) fn readln(binary: &InteractiveBinary) -> Option<String> {
    let prompt = binary.prompt();
    let line =
        binary.context.borrow_mut().read_line(prompt, None, &mut |Event { editor, kind }| {
            let shell = binary.shell.borrow();
            let dirs = &shell.directory_stack;
            let prev = &shell.variables.get::<types::Str>("OLDPWD");
            let vars = &shell.variables;

            if let EventKind::BeforeComplete = kind {
                let (words, pos) = editor.get_words_and_cursor_position();

                let filename = match pos {
                    CursorPosition::InWord(index) => index > 0,
                    CursorPosition::InSpace(Some(_), _) => true,
                    CursorPosition::InSpace(None, _) => false,
                    CursorPosition::OnWordLeftEdge(index) => index >= 1,
                    CursorPosition::OnWordRightEdge(index) => words
                        .into_iter()
                        .nth(index)
                        .map(|(start, end)| editor.current_buffer().range(start, end))
                        .and_then(|filename| {
                            Some(complete_as_file(&env::current_dir().ok()?, &filename, index))
                        })
                        .filter(|&x| x)
                        .is_some(),
                };

                let dir_completer =
                    IonFileCompleter::new(None, dirs, prev.as_ref().map(types::Str::as_str));

                if filename {
                    mem::replace(&mut editor.context().completer, Some(Box::new(dir_completer)));
                } else {
                    // Initialize a new completer from the definitions collected.
                    // Creates a list of definitions from the shell environment that
                    // will be used
                    // in the creation of a custom completer.
                    let custom_completer = BasicCompleter::new(
                        shell
                            .builtins()
                            .keys()
                            // Add built-in commands to the completer's definitions.
                            .map(ToString::to_string)
                            // Add the history list to the completer's definitions.
                            // Map each underlying `liner::Buffer` into a `String`.
                            .chain(editor.context().history.buffers.iter().map(ToString::to_string))
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
                            .collect(),
                    );

                    // Creates completers containing definitions from all directories
                    // listed
                    // in the environment's **$PATH** variable.
                    let mut file_completers: Vec<_> = env::var("PATH")
                        .unwrap_or_else(|_| "/bin/".to_string())
                        .split(sys::PATH_SEPARATOR)
                        .map(|s| {
                            IonFileCompleter::new(
                                Some(s),
                                dirs,
                                prev.as_ref().map(types::Str::as_str),
                            )
                        })
                        .collect();

                    // Also add files/directories in the current directory to the
                    // completion list.
                    file_completers.push(dir_completer);

                    // Merge the collected definitions with the file path definitions.
                    let completer = MultiCompleter::new(file_completers, custom_completer);

                    // Replace the shell's current completer with the newly-created
                    // completer.
                    mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                }
            }
        });

    match line {
        Ok(line) => {
            if line.bytes().next() != Some(b'#') && line.bytes().any(|c| !c.is_ascii_whitespace()) {
                binary.shell.borrow_mut().unterminated = true;
            }
            Some(line)
        }
        // Handles Ctrl + C
        Err(ref err) if err.kind() == ErrorKind::Interrupted => None,
        // Handles Ctrl + D
        Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => {
            let mut shell = binary.shell.borrow_mut();
            if shell.unterminated {
                None
            } else if shell.flow_control.pop() {
                None
            } else {
                let status = shell.previous_status;
                shell.exit(status);
            }
        }
        Err(err) => {
            eprintln!("ion: liner: {}", err);
            None
        }
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
