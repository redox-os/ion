mod assignments;
mod completer;
pub mod directory_stack;
pub mod flags;
pub mod flow_control;
mod flow;
mod history;
mod job;
mod pipe;
pub mod status;
pub mod variables;

pub use self::history::ShellHistory;
pub use self::job::{Job, JobKind};
pub use self::flow::FlowLogic;

use fnv::FnvHashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::env;
use std::mem;
use std::process;
use std::time::SystemTime;

use liner::{Context, CursorPosition, Event, EventKind, FilenameCompleter, BasicCompleter};

use builtins::*;
use self::completer::MultiCompleter;
use self::directory_stack::DirectoryStack;
use self::flow_control::{FlowControl, Function, Statement};
use self::variables::Variables;
use self::status::*;
use self::pipe::execute_pipeline;
use parser::{expand_string, StatementSplitter, check_statement, QuoteTerminator, ExpanderFunctions, Index, IndexEnd};
use parser::peg::Pipeline;

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell<'a> {
    pub builtins: &'a FnvHashMap<&'static str, Builtin>,
    pub context: Context,
    pub variables: Variables,
    flow_control: FlowControl,
    pub directory_stack: DirectoryStack,
    pub functions: FnvHashMap<String, Function>,
    pub previous_status: i32,
    pub flags: u8,
}

impl<'a> Shell<'a> {
    /// Panics if DirectoryStack construction fails
    pub fn new(builtins: &'a FnvHashMap<&'static str, Builtin>) -> Shell<'a> {
        Shell {
            builtins: builtins,
            context: Context::new(),
            variables: Variables::default(),
            flow_control: FlowControl::default(),
            directory_stack: DirectoryStack::new().expect(""),
            functions: FnvHashMap::default(),
            previous_status: 0,
            flags: 0,
        }
    }
    fn readln(&mut self) -> Option<String> {
        let prompt = self.prompt();
        let funcs = &self.functions;
        let vars = &self.variables;
        let builtins = self.builtins;

        // Collects the current list of values from history for completion.
        let history = &self.context.history.buffers.iter()
            // Map each underlying `liner::Buffer` into a `String`.
            .map(|x| x.chars().cloned().collect::<String>())
            // Collect each result into a vector to avoid borrowing issues.
            .collect::<Vec<String>>();

        let line = self.context.read_line(prompt, &mut move |Event { editor, kind }| {
            if let EventKind::BeforeComplete = kind {
                let (words, pos) = editor.get_words_and_cursor_position();

                let filename = match pos {
                    CursorPosition::InWord(index) => index > 0,
                    CursorPosition::InSpace(Some(_), _) => true,
                    CursorPosition::InSpace(None, _) => false,
                    CursorPosition::OnWordLeftEdge(index) => index >= 1,
                    CursorPosition::OnWordRightEdge(index) => {
                        index >= 1 && !words.into_iter().nth(index).map(|(start, end)| {
                            let buf = editor.current_buffer();
                            buf.range(start, end).trim().starts_with('$')
                        }).unwrap_or(false)
                    }
                };

                if filename {
                    if let Ok(current_dir) = env::current_dir() {
                        if let Some(url) = current_dir.to_str() {
                            let completer = FilenameCompleter::new(Some(url));
                            mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                        }
                    }
                } else {
                    // Creates completers containing definitions from all directories listed
                    // in the environment's **$PATH** variable.
                    let file_completers = match env::var("PATH") {
                        Ok(val) => {
                            if cfg!(unix) {
                                // UNIX systems separate paths with the `:` character.
                                val.split(':').map(|x| FilenameCompleter::new(Some(x))).collect::<Vec<_>>()
                            } else {
                                // Redox and Windows use the `;` character to separate paths
                                val.split(';').map(|x| FilenameCompleter::new(Some(x))).collect::<Vec<_>>()
                            }
                        },
                        Err(_) => vec![FilenameCompleter::new(Some("/bin/"))],
                    };

                    // Creates a list of definitions from the shell environment that will be used
                    // in the creation of a custom completer.
                    let words = builtins.iter()
                        // Add built-in commands to the completer's definitions.
                        .map(|(&s, _)| String::from(s))
                        // Add the history list to the completer's definitions.
                        .chain(history.iter().cloned())
                        // Add the aliases to the completer's definitions.
                        .chain(vars.aliases.keys().cloned())
                        // Add the list of available functions to the completer's definitions.
                        .chain(funcs.keys().cloned())
                        // Add the list of available variables to the completer's definitions.
                        .chain(vars.get_vars().into_iter().map(|s| format!("${}", s)))
                        .collect();

                    // Initialize a new completer from the definitions collected.
                    let custom_completer = BasicCompleter::new(words);
                    // Merge the collected definitions with the file path definitions.
                    let completer = MultiCompleter::new(file_completers, custom_completer);

                    // Replace the shell's current completer with the newly-created completer.
                    mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                }
            }
        });

        match line {
            Ok(line) => Some(line),
            Err(err) => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: {}", err);
                None
            }
        }
    }

    pub fn terminate_quotes(&mut self, command: String) -> String {
        let mut buffer = QuoteTerminator::new(command);
        self.flow_control.level += 1;
        while !buffer.check_termination() {
            loop {
                if let Some(command) = self.readln() {
                    buffer.append(command);
                    break
                }
            }
        }
        self.flow_control.level -= 1;
        buffer.consume()
    }

    pub fn execute(&mut self) {
        let mut args = env::args().skip(1);

        if let Some(path) = args.next() {
            if path == "-c" {
                if let Some(mut arg) = args.next() {
                    for argument in args {
                        arg.push(' ');
                        arg.push_str(&argument);
                    }
                    self.on_command(&arg);
                } else {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: -c requires an argument");
                    process::exit(FAILURE);
                }
            } else {
                let mut array = vec![ path.clone() ];
                for arg in args { array.push(arg); }
                self.variables.set_array("args", array);

                match File::open(&path) {
                    Ok(mut file) => {
                        let capacity = file.metadata().ok().map_or(0, |x| x.len());
                        let mut command_list = String::with_capacity(capacity as usize);
                        match file.read_to_string(&mut command_list) {
                            Ok(_) => {
                                let mut lines = command_list.lines().map(|x| x.to_owned());
                                while let Some(command) = lines.next() {
                                    let mut buffer = QuoteTerminator::new(command);
                                    while !buffer.check_termination() {
                                        loop {
                                            if let Some(command) = lines.next() {
                                                buffer.append(command);
                                                break
                                            }
                                        }
                                    }
                                    self.on_command(&buffer.consume());
                                }
                            },
                            Err(err) => {
                                let stderr = io::stderr();
                                let mut stderr = stderr.lock();
                                let _ = writeln!(stderr, "ion: failed to read {}: {}", path, err);
                            }
                        }
                    },
                    Err(err) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: failed to open {}: {}", path, err);
                    }
                }
            }

            process::exit(self.previous_status);
        }

        self.variables.set_array("args", vec![env::args().next().unwrap()]);
        while let Some(command) = self.readln() {
            if ! command.is_empty() {
                let command = self.terminate_quotes(command);
                let command = command.trim();

                // Parse and potentially execute the command.
                self.on_command(command);

                // Mark the command in the context history if it was a success.
                if self.previous_status != NO_SUCH_COMMAND || self.flow_control.level > 0 {
                    self.set_context_history_from_vars();
                    if let Err(err) = self.context.history.push(command.into()) {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: {}", err);
                    }
                }
            }
            self.update_variables();
        }

        // Exit with the previous command's exit status.
        process::exit(self.previous_status);
    }

    /// This function updates variables that need to be kept consistent with each iteration
    /// of the prompt. For example, the PWD variable needs to be updated to reflect changes to the
    /// the current working directory.
    fn update_variables(&mut self) {
        // Update the PWD (Present Working Directory) variable if the current working directory has
        // been updated.
        env::current_dir().ok().map_or_else(|| env::set_var("PWD", "?"), |path| {
            let pwd = self.variables.get_var_or_empty("PWD");
            let pwd = pwd.as_str();
            let current_dir = path.to_str().unwrap_or("?");
            if pwd != current_dir {
                env::set_var("OLDPWD", pwd);
                env::set_var("PWD", current_dir);
            }
        })
    }

    /// Evaluates the source init file in the user's home directory.
    pub fn evaluate_init_file(&mut self) {
        env::home_dir().map_or_else(|| {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(b"ion: could not get home directory");
        }, |mut source_file| {
            source_file.push(".ionrc");
            if let Ok(mut file) = File::open(&source_file) {
                let capacity = file.metadata().map(|x| x.len()).unwrap_or(0) as usize;
                let mut command_list = String::with_capacity(capacity);
                if let Err(message) = file.read_to_string(&mut command_list) {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: {}: failed to read {:?}", message, source_file);
                } else {
                    for command in command_list.lines() {
                        self.on_command(command);
                    }
                }
            }
        });
    }

    pub fn prompt(&self) -> String {
        if self.flow_control.level == 0 {
            let prompt_var = self.variables.get_var_or_empty("PROMPT");
            expand_string(&prompt_var, &get_expanders!(&self.variables, &self.directory_stack), false).join(" ")
        } else {
            "    ".repeat(self.flow_control.level as usize)
        }
    }



    /// Executes a pipeline and returns the final exit status of the pipeline.
    /// To avoid infinite recursion when using aliases, the noalias boolean will be set the true
    /// if an alias branch was executed.
    fn run_pipeline(&mut self, pipeline: &mut Pipeline, noalias: bool) -> Option<i32> {
        let command_start_time = SystemTime::now();

        let mut exit_status = None;
        let mut branched = false;
        let builtins = self.builtins;

        if !noalias {
            if let Some(mut alias) = self.variables.aliases.get(pipeline.jobs[0].command.as_str()).cloned() {
                branched = true;
                // Append arguments supplied by the current job to the alias.
                alias += " ";
                for argument in pipeline.jobs[0].args.iter().skip(1) {
                    alias += " ";
                    alias += argument;
                }

                for statement in StatementSplitter::new(&alias).filter_map(check_statement) {
                    match statement {
                        Statement::Pipeline(mut pipeline) => exit_status = self.run_pipeline(&mut pipeline, true),
                        _ => {
                            exit_status = Some(FAILURE);
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "ion: syntax error: alias only supports pipeline arguments");
                        }
                    }
                }
            }
        }

        if !branched {
            pipeline.expand(&self.variables, &self.directory_stack);
            // Branch if -> input == shell command i.e. echo
            exit_status = if let Some(command) = builtins.get(pipeline.jobs[0].command.as_str()) {
                // Run the 'main' of the command and set exit_status
                if pipeline.jobs.len() == 1 {
                    Some((*command.main)(pipeline.jobs[0].args.as_slice(), self))
                } else {
                    Some(execute_pipeline(pipeline))
                }
            // Branch else if -> input == shell function and set the exit_status
            } else if let Some(function) = self.functions.get(pipeline.jobs[0].command.as_str()).cloned() {
                if pipeline.jobs.len() == 1 {
                    if pipeline.jobs[0].args.len() - 1 == function.args.len() {
                        let mut variables_backup: FnvHashMap<&str, Option<String>> = FnvHashMap::with_capacity_and_hasher (
                            64, Default::default()
                        );
                        for (name, value) in function.args.iter().zip(pipeline.jobs[0].args.iter().skip(1)) {
                            variables_backup.insert(name, self.variables.get_var(name));
                            self.variables.set_var(name, value);
                        }

                        self.execute_statements(function.statements);

                        for (name, value_option) in &variables_backup {
                            match *value_option {
                                Some(ref value) => self.variables.set_var(name, value),
                                None => {self.variables.unset_var(name);},
                            }
                        }
                        None
                    } else {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "This function takes {} arguments, but you provided {}",
                            function.args.len(), pipeline.jobs[0].args.len()-1);
                        Some(NO_SUCH_COMMAND) // not sure if this is the right error code
                    }
                } else {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "Function pipelining is not implemented yet");
                    Some(FAILURE)
                }
            // If not a shell command or a shell function execute the pipeline and set the exit_status
            } else {
                Some(execute_pipeline(pipeline))
            };
        }

        if let Ok(elapsed_time) = command_start_time.elapsed() {
            let summary = format!("#summary# elapsed real time: {}.{:09} seconds",
                                  elapsed_time.as_secs(), elapsed_time.subsec_nanos());

            // If `RECORD_SUMMARY` is set to "1" (True, Yes), then write a summary of the pipline
            // just executed to the the file and context histories. At the moment, this means
            // record how long it took.
            if "1" == self.variables.get_var_or_empty("RECORD_SUMMARY") {
                self.context.history.push(summary.into()).unwrap_or_else(|err| {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: {}\n", err);
                });
            }
        }

        // Retrieve the exit_status and set the $? variable and history.previous_status
        if let Some(code) = exit_status {
            self.variables.set_var("?", &code.to_string());
            self.previous_status = code;
        }
        exit_status
    }


}
