#![deny(warnings)]
#![feature(box_syntax)]
#![feature(plugin)]
#![plugin(peg_syntax_ext)]
extern crate glob;
extern crate liner;

use std::collections::HashMap;
use std::fs::File;
use std::iter;
use std::io::{self, ErrorKind, Read, Write};
use std::env;
use std::mem;
use std::process;
use std::time::SystemTime;

use liner::{Context, CursorPosition, Event, EventKind, FilenameCompleter, BasicCompleter};

use builtins::*;
use completer::MultiCompleter;
use directory_stack::DirectoryStack;
use variables::Variables;
use status::*;
use function::Function;
use pipe::execute_pipeline;
use parser::shell_expand::ExpandErr;
use parser::{expand_string, ForExpression, StatementSplitter};
use parser::peg::{parse, Pipeline};
use flow_control::{FlowControl, Statement, Comparitor};

pub mod completer;
pub mod pipe;
pub mod directory_stack;
pub mod to_num;
pub mod variables;
pub mod status;
pub mod function;
pub mod flow_control;
mod builtins;
mod parser;

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    context: Context,
    variables: Variables,
    flow_control: FlowControl,
    directory_stack: DirectoryStack,
    functions: HashMap<String, Function>,
    previous_status: i32,
}

impl Default for Shell {
    /// Panics if DirectoryStack construction fails
    fn default() -> Shell {
        Shell {
            context: Context::new(),
            variables: Variables::default(),
            flow_control: FlowControl::default(),
            directory_stack: DirectoryStack::new().expect(""),
            functions: HashMap::default(),
            previous_status: 0,
        }
    }
}

impl Shell {
    fn readln(&mut self) -> Option<String> {
        let prompt = self.prompt();
        let funcs = &self.functions;
        let vars = &self.variables;

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
                    let words = Command::map().into_iter()
                        // Add built-in commands to the completer's definitions.
                        .map(|(s, _)| String::from(s))
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

    fn execute(&mut self) {
        let mut dash_c = false;
        for arg in env::args().skip(1) {
            if arg == "-c" {
                dash_c = true;
            } else {
                if dash_c {
                    self.on_command(&arg);
                } else {
                    match File::open(&arg) {
                        Ok(mut file) => {
                            let mut command_list = String::new();
                            match file.read_to_string(&mut command_list) {
                                Ok(_) => {
                                    for command in command_list.lines() {
                                        self.on_command(command);
                                    }
                                },
                                Err(err) => {
                                    let stderr = io::stderr();
                                    let mut stderr = stderr.lock();
                                    let _ = writeln!(stderr, "ion: failed to read {}: {}", arg, err);
                                }
                            }
                        },
                        Err(err) => {
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "ion: failed to open {}: {}", arg, err);
                        }
                    }
                }

                // Exit with the previous command's exit status.
                process::exit(self.previous_status);
            }
        }

        while let Some(command) = self.readln() {
            let command = command.trim();
            if ! command.is_empty() {
                // Mark the command in the context history
                self.set_context_history_from_vars();
                if let Err(err) = self.context.history.push(command.into()) {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: {}", err);
                }

                self.on_command(command);
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
    fn evaluate_init_file(&mut self) {
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
        let mut prompt = self.flow_control.modes.iter().rev().fold(String::new(), |acc, mode| {
            acc +
            if mode.value {
                "+ "
            } else {
                "- "
            }
        }).to_string();

        match self.flow_control.current_statement {
            Statement::For { .. } => {
                prompt.push_str("for> ");
            },
            Statement::While { .. } => {
                prompt.push_str("while> ");
            },
            Statement::Function { .. } => {
                prompt.push_str("fn> ");
            },
            _ => {
                let prompt_var = self.variables.get_var_or_empty("PROMPT");
                match expand_string(&prompt_var, &self.variables, &self.directory_stack) {
                    Ok(ref expanded_string) => prompt.push_str(expanded_string),
                    Err(ExpandErr::UnmatchedBraces(position)) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: expand error: unmatched braces\n{}\n{}^",
                            prompt_var, iter::repeat("-").take(position).collect::<String>());
                        prompt.push_str("ERROR: ");
                    },
                    Err(ExpandErr::InnerBracesNotImplemented) => {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = stderr.write_all(b"ion: expand error: inner braces not yet implemented\n");
                        prompt.push_str("ERROR: ");
                    }
                }
            }
        }

        prompt
    }

    fn on_command(&mut self, command_string: &str) {
        for statement in StatementSplitter::new(command_string).map(parse) {
            if statement.is_flow_control() {
                self.flow_control.current_statement = statement.clone();
                self.flow_control.collecting_block = true;
            }

            match statement {
                Statement::End                         => self.handle_end(),
                Statement::If{left, right, comparitor} => self.handle_if(left, comparitor, right),
                Statement::Pipelines(pipelines)        => { let _ = self.handle_pipelines(pipelines, false); },
                _                                      => {}
            }
        }
    }

    fn handle_if(&mut self, left: String, comparitor: Comparitor, right: String) {
        let left  = expand_string(&left, &self.variables, &self.directory_stack).unwrap_or_else(|_| "".to_string());
        let right = expand_string(&right, &self.variables, &self.directory_stack).unwrap_or_else(|_| "".to_string());

        let value = match comparitor {
            Comparitor::GreaterThan        => { left >  right },
            Comparitor::GreaterThanOrEqual => { left >= right },
            Comparitor::LessThan           => { left <  right },
            Comparitor::LessThanOrEqual    => { left <= right },
            Comparitor::Equal              => { left == right },
            Comparitor::NotEqual           => { left != right },
        };

        self.flow_control.modes.push(flow_control::Mode{value: value})
    }

    fn handle_end(&mut self){
        self.flow_control.collecting_block = false;
        match self.flow_control.current_statement.clone() {
            Statement::While{ref expression} => {
                let block_jobs: Vec<Pipeline> = self.flow_control.current_block
                    .pipelines.drain(..).collect();
                while self.run_pipeline(expression, false) == Some(SUCCESS) {
                    for pipeline in &block_jobs {
                        self.run_pipeline(pipeline, false);
                    }
                }
            },
            Statement::For{variable: ref var, values: ref vals} => {
                let block_jobs: Vec<Pipeline> = self.flow_control
                    .current_block
                    .pipelines
                    .drain(..)
                    .collect();

                match ForExpression::new(vals.as_str(), &self.directory_stack, &self.variables) {
                    ForExpression::Normal(expression) => {
                        for value in expression.split_whitespace() {
                            self.variables.set_var(var, value);
                            for pipeline in &block_jobs {
                                self.run_pipeline(pipeline, false);
                            }
                        }
                    },
                    ForExpression::Range(start, end) => {
                        for value in (start..end).map(|x| x.to_string()) {
                            self.variables.set_var(var, &value);
                            for pipeline in &block_jobs {
                                self.run_pipeline(pipeline, false);
                            }
                        }
                    }
                }
            },
            Statement::Function{ref name, ref args} => {
                    let block_jobs: Vec<Pipeline> = self.flow_control
                        .current_block
                        .pipelines
                        .drain(..)
                        .collect();
                self.functions.insert(name.clone(), Function { name: name.clone(), pipelines: block_jobs.clone(), args: args.clone() });
            },
            Statement::If{..} | Statement::Else => {
                self.flow_control.modes.pop();
                if self.flow_control.modes.is_empty() {
                    let block_jobs: Vec<Pipeline> = self.flow_control
                        .current_block
                        .pipelines
                        .drain(..)
                        .collect();
                    for pipeline in &block_jobs {
                        self.run_pipeline(pipeline, false);
                    }
                }
            },
            _ => {
                    let block_jobs: Vec<Pipeline> = self.flow_control
                        .current_block
                        .pipelines
                        .drain(..)
                        .collect();
                for pipeline in &block_jobs {
                    self.run_pipeline(pipeline, false);
                }
            }
        }
        self.flow_control.current_statement = Statement::Default;
    }

    fn handle_pipelines(&mut self, mut pipelines: Vec<Pipeline>, noalias: bool) -> Option<i32> {
        let mut return_value = None;
        for pipeline in pipelines.drain(..) {
            if self.flow_control.collecting_block {
                let mode = self.flow_control.modes.last().unwrap_or(&flow_control::Mode{value: false}).value;
                match (mode, self.flow_control.current_statement.clone()) {
                    (true, Statement::If{..}) | (false, Statement::Else) | (_, Statement::While{..}) |
                    (_, Statement::For{..}) |(_, Statement::Function{..}) => self.flow_control.current_block.pipelines.push(pipeline),
                    _ => {}
                }
                return_value = None;
            } else {
                return_value = self.run_pipeline(&pipeline, noalias);
            }
        }
        return_value
    }

    /// Sets the history size for the shell context equal to the HISTORY_SIZE shell variable if it
    /// is set otherwise to a default value (1000).
    ///
    /// If the HISTORY_FILE_ENABLED shell variable is set to 1, then HISTORY_FILE_SIZE is synced
    /// with the shell context as well. Otherwise, the history file name is set to None in the
    /// shell context.
    ///
    /// This is called in on_command so that the history length and history file state will be
    /// updated correctly after a command is entered that alters them and just before loading the
    /// history file so that it will be loaded correctly.
    fn set_context_history_from_vars(&mut self) {
        let max_history_size = self.variables
            .get_var_or_empty("HISTORY_SIZE")
            .parse()
            .unwrap_or(1000);

        self.context.history.set_max_size(max_history_size);

        if self.variables.get_var_or_empty("HISTORY_FILE_ENABLED") == "1" {
            let file_name = self.variables.get_var("HISTORY_FILE");
            self.context.history.set_file_name(file_name);

            let max_history_file_size = self.variables
                .get_var_or_empty("HISTORY_FILE_SIZE")
                .parse()
                .unwrap_or(1000);
            self.context.history.set_max_file_size(max_history_file_size);
        } else {
            self.context.history.set_file_name(None);
        }
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    /// To avoid infinite recursion when using aliases, the noalias boolean will be set the true
    /// if an alias branch was executed.
    fn run_pipeline(&mut self, pipeline: &Pipeline, noalias: bool) -> Option<i32> {
        let mut pipeline = self.variables.expand_pipeline(pipeline, &self.directory_stack);
        pipeline.expand_globs();

        let command_start_time = SystemTime::now();

        let mut exit_status = None;
        let mut branched = false;

        if !noalias {
            if let Some(mut alias) = self.variables.aliases.get(pipeline.jobs[0].command.as_str()).cloned() {
                branched = true;
                // Append arguments supplied by the current job to the alias.
                alias += " ";
                for argument in pipeline.jobs[0].args.iter().skip(1) {
                    alias += argument;
                }

                // Execute each statement within the alias and return the last return value.
                for statement in StatementSplitter::new(&alias).map(parse) {
                    if statement.is_flow_control() {
                        self.flow_control.current_statement = statement.clone();
                        self.flow_control.collecting_block = true;
                    }

                    match statement {
                        Statement::End                         => self.handle_end(),
                        Statement::If{left, right, comparitor} => self.handle_if(left, comparitor, right),
                        Statement::Pipelines(pipelines)        => {
                            exit_status = self.handle_pipelines(pipelines, true);
                        },
                        _ => {}
                    }
                }
            }
        }

        if !branched {
            // Branch if -> input == shell command i.e. echo
            exit_status = if let Some(command) = Command::map().get(pipeline.jobs[0].command.as_str()) {
                // Run the 'main' of the command and set exit_status
                Some((*command.main)(pipeline.jobs[0].args.as_slice(), self))
            // Branch else if -> input == shell function and set the exit_status
            } else if let Some(function) = self.functions.get(pipeline.jobs[0].command.as_str()).cloned() {
                if pipeline.jobs[0].args.len() - 1 == function.args.len() {
                    let mut variables_backup: HashMap<&str, Option<String>> = HashMap::new();
                    for (name, value) in function.args.iter().zip(pipeline.jobs[0].args.iter().skip(1)) {
                        variables_backup.insert(name, self.variables.get_var(name));
                        self.variables.set_var(name, value);
                    }
                    let mut return_value = None;
                    for function_pipeline in &function.pipelines {
                        return_value = self.run_pipeline(function_pipeline, false)
                    }
                    for (name, value_option) in &variables_backup {
                        match *value_option {
                            Some(ref value) => self.variables.set_var(name, value),
                            None => {self.variables.unset_var(name);},
                        }
                    }
                    return_value
                } else {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "This function takes {} arguments, but you provided {}",
                        function.args.len(), pipeline.jobs[0].args.len()-1);
                    Some(NO_SUCH_COMMAND) // not sure if this is the right error code
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

    /// Evaluates the given file and returns 'SUCCESS' if it succeeds.
    fn source_command(&mut self, arguments: &[String]) -> Result<(), String> {
        match arguments.get(1) {
            Some(argument) => {
                if let Ok(mut file) = File::open(&argument) {
                    let capacity = file.metadata().map(|x| x.len()).unwrap_or(0) as usize;
                    let mut command_list = String::with_capacity(capacity);
                    file.read_to_string(&mut command_list)
                        .map_err(|message| format!("ion: {}: failed to read {}\n", message, argument))
                        .map(|_| {
                            for command in command_list.lines() { self.on_command(command); }
                            ()
                        })
                } else {
                    Err(format!("ion: failed to open {}\n", argument))
                }
            },
            None => {
                self.evaluate_init_file();
                Ok(())
            },
        }
    }

    fn print_history(&self, _arguments: &[String]) -> i32 {
        let mut buffer = Vec::with_capacity(8*1024);
        for command in &self.context.history.buffers {
            let _ = writeln!(buffer, "{}", command);
        }
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        let _ = stdout.write_all(&buffer);
        SUCCESS
    }
}

/// Structure which represents a Terminal's command.
/// This command structure contains a name, and the code which run the
/// functionnality associated to this one, with zero, one or several argument(s).
/// # Example
/// ```
/// let my_command = Command {
///     name: "my_command",
///     help: "Describe what my_command does followed by a newline showing usage",
///     main: box|args: &[String], &mut Shell| -> i32 {
///         println!("Say 'hello' to my command! :-D");
///     }
/// }
/// ```
pub struct Command {
    pub name: &'static str,
    pub help: &'static str,
    pub main: Box<Fn(&[String], &mut Shell) -> i32>,
}

impl Command {
    /// Return the map from command names to commands
    pub fn map() -> HashMap<&'static str, Self> {
        let mut commands: HashMap<&str, Self> = HashMap::new();

        /* Directories */
        commands.insert("cd",
                        Command {
                            name: "cd",
                            help: "Change the current directory\n    cd <path>",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.directory_stack.cd(args, &shell.variables) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }
                            },
                        });

        commands.insert("dirs",
                        Command {
                            name: "dirs",
                            help: "Display the current directory stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.directory_stack.dirs(args)
                            },
                        });

        commands.insert("pushd",
                        Command {
                            name: "pushd",
                            help: "Push a directory to the stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.directory_stack.pushd(args, &shell.variables) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }
                            },
                        });

        commands.insert("popd",
                        Command {
                            name: "popd",
                            help: "Pop a directory from the stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.directory_stack.popd(args) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }
                            },
                        });

        /* Aliases */
        commands.insert("alias",
                        Command {
                            name: "alias",
                            help: "View, set or unset aliases",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                alias(&mut shell.variables, args)
                            },
                        });

        commands.insert("unalias",
                        Command {
                            name: "drop",
                            help: "Delete an alias",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                drop_alias(&mut shell.variables, args)
                            },
                        });

        /* Variables */
        commands.insert("export",
                        Command {
                            name: "export",
                            help: "Set an environment variable",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                export_variable(&mut shell.variables, args)
                            }
                        });

        commands.insert("let",
                        Command {
                            name: "let",
                            help: "View, set or unset variables",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                let_(&mut shell.variables, args)
                            },
                        });

        commands.insert("read",
                        Command {
                            name: "read",
                            help: "Read some variables\n    read <variable>",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.variables.read(args)
                            },
                        });

        commands.insert("drop",
                        Command {
                            name: "drop",
                            help: "Delete a variable",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                drop_variable(&mut shell.variables, args)
                            },
                        });

        /* Misc */
        commands.insert("exit",
                Command {
                    name: "exit",
                    help: "To exit the curent session",
                    main: box |args: &[String], shell: &mut Shell| -> i32 {
                        process::exit(args.get(1).and_then(|status| status.parse::<i32>().ok())
                            .unwrap_or(shell.previous_status))
                    },
                });

        commands.insert("history",
                        Command {
                            name: "history",
                            help: "Display a log of all commands previously executed",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.print_history(args)
                            },
                        });

        commands.insert("source",
                        Command {
                            name: "source",
                            help: "Evaluate the file following the command or re-initialize the init file",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                match shell.source_command(args) {
                                    Ok(()) => SUCCESS,
                                    Err(why) => {
                                        let stderr = io::stderr();
                                        let mut stderr = stderr.lock();
                                        let _ = stderr.write_all(why.as_bytes());
                                        FAILURE
                                    }
                                }

                            },
                        });

        commands.insert("true",
                        Command {
                            name: "true",
                            help: "Do nothing, successfully",
                            main: box |_: &[String], _: &mut Shell| -> i32 {
                                status::SUCCESS
                            },
                        });

        commands.insert("false",
                        Command {
                            name: "false",
                            help: "Do nothing, unsuccessfully",
                            main: box |_: &[String], _: &mut Shell| -> i32 {
                                status::FAILURE
                            },
                        });

        let command_helper: HashMap<&'static str, &'static str> = commands.iter()
                                                                          .map(|(k, v)| {
                                                                              (*k, v.help)
                                                                          })
                                                                          .collect();

        commands.insert("help",
                        Command {
                            name: "help",
                            help: "Display helpful information about a given command, or list \
                                   commands if none specified\n    help <command>",
                            main: box move |args: &[String], _: &mut Shell| -> i32 {
                                let stdout = io::stdout();
                                let mut stdout = stdout.lock();
                                if let Some(command) = args.get(1) {
                                    if command_helper.contains_key(command.as_str()) {
                                        if let Some(help) = command_helper.get(command.as_str()) {
                                            let _ = stdout.write_all(help.as_bytes());
                                            let _ = stdout.write_all(b"\n");
                                        }
                                    }
                                    let _ = stdout.write_all(b"Command helper not found [run 'help']...");
                                    let _ = stdout.write_all(b"\n");
                                } else {
                                    let mut commands = command_helper.keys().cloned().collect::<Vec<&str>>();
                                    commands.sort();

                                    let mut buffer: Vec<u8> = Vec::new();
                                    for command in commands {
                                        let _ = writeln!(buffer, "{}", command);
                                    }
                                    let _ = stdout.write_all(&buffer);
                                }
                                SUCCESS
                            },
                        });

        commands
    }
}

fn main() {
    let mut shell = Shell::default();
    shell.evaluate_init_file();
    // Clear the history just added by the init file being evaluated.
    shell.context.history.buffers.clear();
    shell.set_context_history_from_vars();

    if "1" == shell.variables.get_var_or_empty("HISTORY_FILE_ENABLED") {
        match shell.context.history.load_history() {
            Ok(()) => {
                // pass
            }
            Err(ref err) if err.kind() == ErrorKind::NotFound => {
                let history_filename = shell.variables.get_var_or_empty("HISTORY_FILE");
                println!("ion: failed to find history file {}: {}", history_filename, err);
            },
            Err(err) => {
                println!("failed here!");
                println!("ion: {}", err);
            }
        }
    }
    shell.execute();
}
