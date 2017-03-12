mod history;

pub use self::history::ShellHistory;

use std::collections::HashMap;
use std::fs::File;
use std::iter;
use std::io::{self, Read, Write};
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
use pipe::execute_pipeline;
use parser::shell_expand::ExpandErr;
use parser::{expand_string, ForExpression, StatementSplitter};
use parser::peg::{parse, Pipeline};
use flow_control::{ElseIf, FlowControl, Function, Statement, collect_loops, collect_if};

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    pub context: Context,
    pub variables: Variables,
    flow_control: FlowControl,
    pub directory_stack: DirectoryStack,
    functions: HashMap<String, Function>,
    pub previous_status: i32,
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
                    let words = Builtin::map().into_iter()
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

    pub fn execute(&mut self) {
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
            match expand_string(&prompt_var, &self.variables, &self.directory_stack) {
                Ok(expanded_string) => expanded_string,
                Err(ExpandErr::UnmatchedBraces(position)) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: expand error: unmatched braces\n{}\n{}^",
                        prompt_var, iter::repeat("-").take(position).collect::<String>());
                    String::from("ERROR: ")
                },
                Err(ExpandErr::InnerBracesNotImplemented) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = stderr.write_all(b"ion: expand error: inner braces not yet implemented\n");
                    String::from("ERROR: ")
                }
            }
        } else {
            "    ".repeat(self.flow_control.level as usize)
        }
    }

    fn execute_statements(&mut self, mut statements: Vec<Statement>) -> bool {
        let mut iterator = statements.drain(..);
        while let Some(statement) = iterator.next() {
            match statement {
                Statement::While { expression, mut statements } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.execute_while(expression, statements);
                },
                Statement::For { variable, values, mut statements } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.execute_for(&variable, &values, statements);
                },
                Statement::If { expression, mut success, mut else_if, mut failure } => {
                    self.flow_control.level += 1;
                    if let Err(why) = collect_if(&mut iterator, &mut success, &mut else_if,
                        &mut failure, &mut self.flow_control.level, 0) {
                            let stderr = io::stderr();
                            let mut stderr = stderr.lock();
                            let _ = writeln!(stderr, "{}", why);
                            self.flow_control.level = 0;
                            self.flow_control.current_if_mode = 0;
                            return true
                        }
                    if self.execute_if(expression, success, else_if, failure) {
                        return true
                    }
                },
                Statement::Function { name, args, mut statements } => {
                    self.flow_control.level += 1;
                    collect_loops(&mut iterator, &mut statements, &mut self.flow_control.level);
                    self.functions.insert(name.clone(), Function {
                        name:       name,
                        args:       args,
                        statements: statements
                    });
                },
                Statement::Pipelines(mut pipelines) => {
                    for pipeline in pipelines.drain(..) {
                        self.run_pipeline(&pipeline, false);
                    }
                },
                Statement::Break => {
                    return true
                }
                _ => {}
            }
        }
        false
    }

    fn execute_while(&mut self, expression: Pipeline, statements: Vec<Statement>) {
        while self.run_pipeline(&expression, false) == Some(SUCCESS) {
            // Cloning is needed so the statement can be re-iterated again if needed.
            if self.execute_statements(statements.clone()) {
              break
            }
        }
    }

    fn execute_for(&mut self, variable: &str, values: &str, statements: Vec<Statement>) {
        match ForExpression::new(values, &self.directory_stack, &self.variables) {
            ForExpression::Normal(expression) => {
                for value in expression.split_whitespace() {
                    self.variables.set_var(variable, value);
                    if self.execute_statements(statements.clone()) {
                      break
                    }
                }
            },
            ForExpression::Range(start, end) => {
                for value in (start..end).map(|x| x.to_string()) {
                    self.variables.set_var(variable, &value);
                    if self.execute_statements(statements.clone()) {
                      break
                    }
                }
            }
        }
    }

    fn execute_if(&mut self, expression: Pipeline, success: Vec<Statement>,
        mut else_if: Vec<ElseIf>, failure: Vec<Statement>) -> bool
    {
        match self.run_pipeline(&expression, false) {
            Some(SUCCESS) => self.execute_statements(success),
            _             => {
                for elseif in else_if.drain(..) {
                    if self.run_pipeline(&elseif.expression, false) == Some(SUCCESS) {
                        return self.execute_statements(elseif.success);
                    }
                }
                self.execute_statements(failure)
            }
        }
    }

    fn execute_toplevel<I>(&mut self, iterator: &mut I, statement: Statement) -> Result<(), &'static str>
        where I: Iterator<Item = Statement>
    {
        match statement {
            // Collect the statements for the while loop, and if the loop is complete,
            // execute the while loop with the provided expression.
            Statement::While { expression, mut statements } => {
                self.flow_control.level += 1;

                // Collect all of the statements contained within the while block.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_while(expression, statements);
                } else {
                    // Store the partial `Statement::While` to memory
                    self.flow_control.current_statement = Statement::While {
                        expression: expression,
                        statements: statements,
                    }
                }
            },
            // Collect the statements for the for loop, and if the loop is complete,
            // execute the for loop with the provided expression.
            Statement::For { variable, values, mut statements } => {
                self.flow_control.level += 1;

                // Collect all of the statements contained within the while block.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_for(&variable, &values, statements);
                } else {
                    // Store the partial `Statement::For` to memory
                    self.flow_control.current_statement = Statement::For {
                        variable:   variable,
                        values:     values,
                        statements: statements,
                    }
                }
            },
            // Collect the statements needed for the `success`, `else_if`, and `failure`
            // conditions; then execute the if statement if it is complete.
            Statement::If { expression, mut success, mut else_if, mut failure } => {
                self.flow_control.level += 1;

                // Collect all of the success and failure statements within the if condition.
                // The `mode` value will let us know whether the collector ended while
                // collecting the success block or the failure block.
                let mode = collect_if(iterator, &mut success, &mut else_if,
                    &mut failure, &mut self.flow_control.level, 0)?;

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can immediately execute now
                    self.execute_if(expression, success, else_if, failure);
                } else {
                    // Set the mode and partial if statement in memory.
                    self.flow_control.current_if_mode = mode;
                    self.flow_control.current_statement = Statement::If {
                        expression: expression,
                        success:    success,
                        else_if:    else_if,
                        failure:    failure
                    };
                }
            },
            // Collect the statements needed by the function and add the function to the
            // list of functions if it is complete.
            Statement::Function { name, args, mut statements } => {
                self.flow_control.level += 1;

                // The same logic that applies to loops, also applies here.
                collect_loops(iterator, &mut statements, &mut self.flow_control.level);

                if self.flow_control.level == 0 {
                    // All blocks were read, thus we can add it to the list
                    self.functions.insert(name.clone(), Function {
                        name:       name,
                        args:       args,
                        statements: statements
                    });
                } else {
                    // Store the partial function declaration in memory.
                    self.flow_control.current_statement = Statement::Function {
                        name:       name,
                        args:       args,
                        statements: statements
                    }
                }
            },
            // Simply executes a provide pipeline, immediately.
            Statement::Pipelines(mut pipelines) => {
                // Immediately execute the command as it has no dependents.
                for pipeline in pipelines.drain(..) {
                    let _ = self.run_pipeline(&pipeline, false);
                }
            },
            // At this level, else and else if keywords are forbidden.
            Statement::ElseIf{..} | Statement::Else => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: syntax error: not an if statement");
            },
            // Likewise to else and else if, the end keyword does nothing here.
            Statement::End => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: syntax error: no block to end");
            },
            _ => {}
        }
        Ok(())
    }

    pub fn on_command(&mut self, command_string: &str) {
        let mut iterator = StatementSplitter::new(command_string).map(parse);

        // If the value is set to `0`, this means that we don't need to append to an existing
        // partial statement block in memory, but can read and execute new statements.
        if self.flow_control.level == 0 {
            while let Some(statement) = iterator.next() {
                // Executes all statements that it can, and stores the last remaining partial
                // statement in memory if needed. We can tell if there is a partial statement
                // later if the value of `level` is not set to `0`.
                if let Err(why) = self.execute_toplevel(&mut iterator, statement) {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "{}", why);
                    self.flow_control.level = 0;
                    self.flow_control.current_if_mode = 0;
                    return
                }
            }
        } else {
            // Appends the newly parsed statements onto the existing statement stored in memory.
            match self.flow_control.current_statement {
                Statement::While{ ref mut statements, .. }
                    | Statement::For { ref mut statements, .. }
                    | Statement::Function { ref mut statements, .. } =>
                {
                    collect_loops(&mut iterator, statements, &mut self.flow_control.level);
                },
                Statement::If { ref mut success, ref mut else_if, ref mut failure, .. } => {
                    self.flow_control.current_if_mode = match collect_if(&mut iterator, success,
                        else_if, failure, &mut self.flow_control.level,
                        self.flow_control.current_if_mode) {
                            Ok(mode) => mode,
                            Err(why) => {
                                let stderr = io::stderr();
                                let mut stderr = stderr.lock();
                                let _ = writeln!(stderr, "{}", why);
                                4
                            }
                        };
                }
                _ => ()
            }

            // If this is true, an error occurred during the if statement
            if self.flow_control.current_if_mode == 4 {
                self.flow_control.level = 0;
                self.flow_control.current_if_mode = 0;
                self.flow_control.current_statement = Statement::Default;
                return
            }

            // If the level is set to 0, it means that the statement in memory is finished
            // and thus is ready for execution.
            if self.flow_control.level == 0 {
                // Replaces the `current_statement` with a `Default` value to avoid the
                // need to clone the value, and clearing it at the same time.
                let mut replacement = Statement::Default;
                mem::swap(&mut self.flow_control.current_statement, &mut replacement);

                match replacement {
                    Statement::While { expression, statements } => {
                        self.execute_while(expression, statements);
                    },
                    Statement::For { variable, values, statements } => {
                        self.execute_for(&variable, &values, statements);
                    },
                    Statement::Function { name, args, statements } => {
                        self.functions.insert(name.clone(), Function {
                            name:       name,
                            args:       args,
                            statements: statements
                        });
                    },
                    Statement::If { expression, success, else_if, failure } => {
                        self.execute_if(expression, success, else_if, failure);
                    }
                    _ => ()
                }

                // Capture any leftover statements.
                while let Some(statement) = iterator.next() {
                    if let Err(why) = self.execute_toplevel(&mut iterator, statement) {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "{}", why);
                        self.flow_control.level = 0;
                        self.flow_control.current_if_mode = 0;
                        return
                    }
                }
            }
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

                for statement in StatementSplitter::new(&alias).map(parse) {
                    match statement {
                        Statement::Pipelines(mut pipelines) => for pipeline in pipelines.drain(..) {
                            exit_status = self.run_pipeline(&pipeline, true);
                        },
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
            // Branch if -> input == shell command i.e. echo
            exit_status = if let Some(command) = Builtin::map().get(pipeline.jobs[0].command.as_str()) {
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
