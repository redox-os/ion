#![deny(warnings)]
#![feature(deque_extras)]
#![feature(box_syntax)]
#![feature(plugin)]
#![plugin(peg_syntax_ext)]

extern crate glob;
extern crate liner;

use std::collections::HashMap;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::env;
use std::mem;
use std::process;
use std::time::SystemTime;

use liner::{Context, CursorPosition, Event, EventKind, FilenameCompleter};

use self::directory_stack::DirectoryStack;
use self::peg::{parse, Pipeline};
use self::variables::Variables;
use self::flow_control::{FlowControl, Statement, Comparitor};
use self::status::{SUCCESS, NO_SUCH_COMMAND};
use self::function::Function;
use self::pipe::execute_pipeline;

pub mod pipe;
pub mod directory_stack;
pub mod to_num;
pub mod peg;
pub mod variables;
pub mod flow_control;
pub mod status;
pub mod function;

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

        let line = self.context.read_line(prompt,
                                          &mut |Event { editor, kind }| {
            match kind {
                EventKind::BeforeComplete => {
                    let (_, pos) = editor.get_words_and_cursor_position();

                    let filename = match pos {
                        CursorPosition::InWord(i) => i > 0,
                        CursorPosition::InSpace(Some(_), _) => true,
                        CursorPosition::InSpace(None, _) => false,
                        CursorPosition::OnWordLeftEdge(i) => i >= 1,
                        CursorPosition::OnWordRightEdge(i) => i >= 1,
                    };

                    if filename {
                        let pathbuf = env::current_dir().unwrap();
                        let url = pathbuf.to_str().unwrap();
                        //HACK FOR LINER LOOKUP ON REDOX
                        let reference = match url.find(':') {
                            Some(i) => &url[i + 1..],
                            None => url
                        };
                        let completer = FilenameCompleter::new(Some(reference));
                        mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                    } else {
                        let completer = FilenameCompleter::new(Some("/bin/"));
                        mem::replace(&mut editor.context().completer, Some(Box::new(completer)));
                    }
                }
                _ => (),
            }
        });

        match line {
            Ok(line) => Some(line),
            Err(err) => {
                println!("ion: {}", err);
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
                                    for command in command_list.split('\n') {
                                        self.on_command(command);
                                    }
                                },
                                Err(err) => println!("ion: failed to read {}: {}", arg, err)
                            }
                        },
                        Err(err) => println!("ion: failed to open {}: {}", arg, err)
                    }
                }

                // Exit with the previous command's exit status.
                process::exit(self.previous_status);
            }
        }

        while let Some(command) = self.readln() {
            let command = command.trim();
            if ! command.is_empty() {
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
        env::home_dir().map_or_else(|| println!("ion: could not get home directory"), |mut source_file| {
            source_file.push(".ionrc");
            if let Ok(mut file) = File::open(&source_file) {
                let mut command_list = String::new();
                if let Err(message) = file.read_to_string(&mut command_list) {
                    println!("{}: Failed to read {:?}", message, source_file)
                } else {
                    self.on_command(&command_list)
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
            Statement::Function { .. } => {
                prompt.push_str("fn> ");
            },
            _ => {
                prompt.push_str(&format!("{}", self.variables.expand_string(&self.variables.get_var_or_empty("PROMPT"), &self.directory_stack)));
            }
        }

        prompt
    }

    fn on_command(&mut self, command_string: &str) {
        self.set_context_history_from_vars();

        let trimmed_command = command_string.trim();
        if !trimmed_command.is_empty() {
            if let Err(err) = self.context.history.push(trimmed_command.into()) {
                println!("ion: {}", err);
            }
        }

        let update = parse(command_string);
        if update.is_flow_control() {
            self.flow_control.current_statement = update.clone();
            self.flow_control.collecting_block = true;
        }

        match update {
            Statement::End                         => self.handle_end(),
            Statement::If{left, right, comparitor} => self.handle_if(left, comparitor, right),
            Statement::Pipelines(pipelines)        => self.handle_pipelines(pipelines),
            _                                      => {}
        }

    }

    fn handle_if(&mut self, left: String, comparitor: Comparitor, right: String) {
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
            Statement::For{variable: ref var, values: ref vals} => {
                    let block_jobs: Vec<Pipeline> = self.flow_control
                        .current_block
                        .pipelines
                        .drain(..)
                        .collect();
                let variable = var.clone();
                let values = vals.clone();
                for value in values {
                    self.variables.set_var(&variable, &value);
                    for pipeline in &block_jobs {
                        self.run_pipeline(pipeline);
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
            Statement::If{..} => {
                self.flow_control.modes.pop();
                if self.flow_control.modes.is_empty() {
                    let block_jobs: Vec<Pipeline> = self.flow_control
                        .current_block
                        .pipelines
                        .drain(..)
                        .collect();
                    for pipeline in &block_jobs {
                        self.run_pipeline(pipeline);
                    }
                }
            },
            Statement::Else => {
                self.flow_control.modes.pop();
                if self.flow_control.modes.is_empty() {
                    let block_jobs: Vec<Pipeline> = self.flow_control
                        .current_block
                        .pipelines
                        .drain(..)
                        .collect();
                    for pipeline in &block_jobs {
                        self.run_pipeline(pipeline);
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
                    self.run_pipeline(pipeline);
                }
            }
        }
        self.flow_control.current_statement = Statement::Default;
    }

    fn handle_pipelines(&mut self, mut pipelines: Vec<Pipeline>) {
        for pipeline in pipelines.drain(..) {
            if self.flow_control.collecting_block {
                let mode = self.flow_control.modes.last().unwrap_or(&flow_control::Mode{value: false}).value;
                match (mode, self.flow_control.current_statement.clone()) {
                    (true, Statement::If{..}) | (false, Statement::Else) |
                    (_, Statement::For{..}) |(_, Statement::Function{..}) => self.flow_control.current_block.pipelines.push(pipeline),
                    _ => {}
                }
            } else {
                self.run_pipeline(&pipeline);
            }
        }
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

    fn run_pipeline(&mut self, pipeline: &Pipeline) -> Option<i32> {
        let mut pipeline = self.variables.expand_pipeline(pipeline, &self.directory_stack);
        pipeline.expand_globs();

        let command_start_time = SystemTime::now();

        // Branch if -> input == shell command i.e. echo
        // Run the 'main' of the command and set exit_status
        let exit_status = if let Some(command) = Command::map().get(pipeline.jobs[0].command.as_str()) {
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
                    return_value = self.run_pipeline(function_pipeline)
                }
                for (name, value_option) in &variables_backup {
                    match *value_option {
                        Some(ref value) => self.variables.set_var(name, value),
                        None => {self.variables.unset_var(name);},
                    }
                }
                return_value
            } else {
                println!("This function takes {} arguments, but you provided {}", function.args.len(), pipeline.jobs[0].args.len()-1);
                Some(NO_SUCH_COMMAND) // not sure if this is the right error code
            }
        // If not a shell command or a shell function execute the pipeline and set the exit_status
        } else {
            Some(execute_pipeline(pipeline))
        };

        if let Some(elapsed_time) = command_start_time.elapsed().ok() {
            let summary = format!("#summary# elapsed real time: {}.{:09} seconds",
                                  elapsed_time.as_secs(), elapsed_time.subsec_nanos()).into();

            // If `RECORD_SUMMARY` is set to "1" (True, Yes), then write a summary of the pipline
            // just executed to the the file and context histories. At the moment, this means
            // record how long it took.
            if "1".to_string() == self.variables.get_var_or_empty("RECORD_SUMMARY") {
                if let Err(err) = self.context.history.push(summary) {
                    println!("ion: {}", err);
                }
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
    fn source_command(&mut self, arguments: &[String]) -> i32 {
        match arguments.iter().skip(1).next() {
            Some(argument) => {
                if let Ok(mut file) = File::open(&argument) {
                    let mut command_list = String::new();
                    if let Err(message) = file.read_to_string(&mut command_list) {
                        println!("{}: Failed to read {}", message, argument);
                        status::FAILURE
                    } else {
                        self.on_command(&command_list);
                        status::SUCCESS
                    }
                } else {
                    println!("Failed to open {}", argument);
                    status::FAILURE
                }
            },
            None => {
                self.evaluate_init_file();
                status::SUCCESS
            },
        }
    }

    fn print_history(&self, _arguments: &[String]) -> i32 {
        for command in &self.context.history.buffers {
            println!("{}", command);
        }
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

        commands.insert("cd",
                        Command {
                            name: "cd",
                            help: "Change the current directory\n    cd <path>",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.directory_stack.cd(args, &shell.variables)
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

        commands.insert("exit",
                        Command {
                            name: "exit",
                            help: "To exit the curent session",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                process::exit(args.get(1).and_then(|status| status.parse::<i32>().ok())
                                    .unwrap_or(shell.previous_status))
                            },
                        });

        commands.insert("let",
                        Command {
                            name: "let",
                            help: "View, set or unset variables",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.variables.let_(args)
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

        commands.insert("pushd",
                        Command {
                            name: "pushd",
                            help: "Push a directory to the stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.directory_stack.pushd(args, &shell.variables)
                            },
                        });

        commands.insert("popd",
                        Command {
                            name: "popd",
                            help: "Pop a directory from the stack",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.directory_stack.popd(args)
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
                                shell.source_command(args)

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


        commands.insert("drop",
                        Command {
                            name: "drop",
                            help: "Delete a variable",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.variables.drop_variable(args)
                            },
                        });

        commands.insert("export",
                        Command {
                            name: "export",
                            help: "Set an environment variable",
                            main: box |args: &[String], shell: &mut Shell| -> i32 {
                                shell.variables.export_variable(args)
                            }
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
                                if let Some(command) = args.get(1) {
                                    if command_helper.contains_key(command.as_str()) {
                                        match command_helper.get(command.as_str()) {
                                            Some(help) => println!("{}", help),
                                            None => {
                                                println!("Command helper not found [run 'help']...")
                                            }
                                        }
                                    } else {
                                        println!("Command helper not found [run 'help']...");
                                    }
                                } else {
                                    for command in command_helper.keys() {
                                        println!("{}", command);
                                    }
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
                ();
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
