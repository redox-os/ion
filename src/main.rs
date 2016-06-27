#![feature(deque_extras)]
#![feature(box_syntax)]
#![feature(plugin)]
#![plugin(peg_syntax_ext)]

extern crate glob;

use std::collections::HashMap;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::env;
use std::process;

use self::directory_stack::DirectoryStack;
use self::input_editor::readln;
use self::peg::{parse, Pipeline};
use self::variables::Variables;
use self::history::History;
use self::flow_control::{FlowControl, is_flow_control_command, Statement, Comparitor};
use self::status::{SUCCESS, NO_SUCH_COMMAND};
use self::function::Function;
use self::pipe::execute_pipeline;

pub mod pipe;
pub mod directory_stack;
pub mod to_num;
pub mod input_editor;
pub mod peg;
pub mod variables;
pub mod history;
pub mod flow_control;
pub mod status;
pub mod function;

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    variables: Variables,
    flow_control: FlowControl,
    directory_stack: DirectoryStack,
    history: History,
    functions: HashMap<String, Function>,
}

impl Default for Shell {
    /// Panics if DirectoryStack construction fails
    fn default() -> Shell {
        let mut new_shell = Shell {
            variables: Variables::default(),
            flow_control: FlowControl::default(),
            directory_stack: DirectoryStack::new().expect(""),
            history: History::default(),
            functions: HashMap::new(),
        };
        new_shell.initialize_default_variables();
        new_shell.evaluate_init_file();
        new_shell
    }
}

impl Shell {
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
                                Ok(_) => self.on_command(&command_list),
                                Err(err) => println!("ion: failed to read {}: {}", arg, err)
                            }
                        },
                        Err(err) => println!("ion: failed to open {}: {}", arg, err)
                    }
                }

                // Exit with the previous command's exit status.
                process::exit(self.history.previous_status);
            }
        }

        self.print_prompt();
        while let Some(command) = readln() {
            let command = command.trim();
            if !command.is_empty() {
                self.on_command(command);
            }
            self.update_variables();
            self.print_prompt();
        }

        // Exit with the previous command's exit status.
        process::exit(self.history.previous_status);
    }

    /// This function will initialize the default variables used by the shell. This function will
    /// be called before evaluating the init
    fn initialize_default_variables(&mut self) {
        self.variables.set_var("DIRECTORY_STACK_SIZE", "1000");
        self.variables.set_var("HISTORY_SIZE", "1000");
        self.variables.set_var("HISTORY_FILE_ENABLED", "0");
        self.variables.set_var("HISTORY_FILE_SIZE", "1000");
        self.variables.set_var("PROMPT", "\x1B[0m\x1B[1;38;5;85mion\x1B[37m:\x1B[38;5;75m$PWD\x1B[37m#\x1B[0m ");

        if let Some(mut history_path) = std::env::home_dir() {   // Initialize the HISTORY_FILE variable
            history_path.push(".ion_history");
            self.variables.set_var("HISTORY_FILE", history_path.to_str().unwrap_or("?"));
        }

        // Initialize the PWD (Present Working Directory) variable
        match std::env::current_dir() {
            Ok(path) => env::set_var("PWD", path.to_str().unwrap_or("?")),
            Err(_)   => env::set_var("PWD", "?")
        }

        // Initialize the HOME variable
        match std::env::home_dir() {
            Some(path) => env::set_var("HOME", path.to_str().unwrap_or("?")),
            None       => env::set_var("HOME", "?")
        }
    }

    /// This functional will update variables that need to be kept consistent with each iteration
    /// of the prompt. In example, the PWD variable needs to be updated to reflect changes to the
    /// the current working directory.
    fn update_variables(&mut self) {
        // Update the PWD (Present Working Directory) variable if the current working directory has
        // been updated.
        match std::env::current_dir() {
            Ok(path) => {
                let pwd = self.variables.get_var_or_empty("PWD");
                let pwd = pwd.as_str();
                let current_dir = path.to_str().unwrap_or("?");
                if pwd != current_dir {
                    env::set_var("OLDPWD", pwd);
                    env::set_var("PWD", current_dir);
                }
            }
            Err(_) => env::set_var("PWD", "?"),
        }

    }

    /// Evaluates the source init file in the user's home directory.
    fn evaluate_init_file(&mut self) {

        // Obtain home directory
        if let Some(mut source_file) = std::env::home_dir() {
            // Location of ion init file
            source_file.push(".ionrc");

            if let Ok(mut file) = File::open(source_file.clone()) {
                let mut command_list = String::new();
                if let Err(message) = file.read_to_string(&mut command_list) {
                    println!("{}: Failed to read {:?}", message, source_file.clone());
                } else {
                    self.on_command(&command_list);
                }
            }
        } else {
            println!("ion: could not get home directory");
        }
    }

    pub fn print_prompt(&self) {
        self.print_prompt_prefix();
        match self.flow_control.current_statement {
            Statement::For{..} => self.print_for_prompt(),
            Statement::Function{..} => self.print_function_prompt(),
            _ => self.print_default_prompt(),
        }
        if let Err(message) = stdout().flush() {
            println!("{}: failed to flush prompt to stdout", message);
        }

    }

    // TODO eventually this thing should be gone
    fn print_prompt_prefix(&self) {
        let prompt_prefix = self.flow_control.modes.iter().rev().fold(String::new(), |acc, mode| {
            acc +
            if mode.value {
                "+ "
            } else {
                "- "
            }
        });
        print!("{}", prompt_prefix);
    }

    fn print_for_prompt(&self) {
        print!("for> ");
    }

    fn print_function_prompt(&self) {
        print!("fn> ");
    }

    fn print_default_prompt(&self) {
        print!("{}",
               self.variables.expand_string(&self.variables.get_var_or_empty("PROMPT"),
                                            &self.directory_stack));
    }

    fn on_command(&mut self, command_string: &str) {
        self.history.add(command_string.to_string(), &self.variables);

        let update = parse(command_string);

        match update {
            Statement::End => self.handle_end(),
            Statement::If{left, right, comparitor} => self.handle_if(left, comparitor, right),
            //Statement::Else => handle_else,
            //Statement::For{variable: v, values: vs} => handle_for,
            //Statement::Function{name: name, args: args} => handle_func,
            Statement::Pipelines(pipelines) => self.handle_pipelines(pipelines),
            _ => {}
        }

    }

    fn handle_if(&mut self, left: String, comparitor: Comparitor, right: String) {
        let value = match comparitor {
            Comparitor::GreaterThan        => { left >  right },
            Comparitor::GreaterThanOrEqual => { left >= right },
            Comparitor::LessThan           => { left <  right },
            Comparitor::LessThanOrEqual    => { left <= right },
            Comparitor::Equal              => { left == right },
            Comparitor::NotEqual           => { left == right },
        };

        self.flow_control.collecting_block = true;
        self.flow_control.modes.insert(0, flow_control::Mode{value: value})
    }

    fn handle_end(&mut self){
        self.flow_control.collecting_block = false;
        let block_jobs: Vec<Pipeline> = self.flow_control
            .current_block
            .pipelines
            .drain(..)
            .collect();
        match self.flow_control.current_statement.clone() {
            //Statement::For{variable: ref var, values: ref vals} => {
                //let variable = var.clone();
                //let values = vals.clone();
                //for value in values {
                    //self.variables.set_var(&variable, &value);
                    //for pipeline in &block_jobs {
                        //self.run_pipeline(&pipeline);
                    //}
                //}
            //},
            //Statement::Function{ref name, ref args} => {
                //self.functions.insert(name.clone(), Function { name: name.clone(), pipelines: block_jobs.clone(), args: args.clone() });
            //},
            _ => {
                if let Some(&flow_control::Mode{value: true}) = self.flow_control.modes.get(0) {
                for pipeline in &block_jobs {
                    self.run_pipeline(&pipeline);
                }
                }
                self.flow_control.modes.clear();
            }
        }
        self.flow_control.current_statement = Statement::Default;
    }

    fn handle_pipelines(&mut self, mut pipelines: Vec<Pipeline>) {
        for pipeline in pipelines.drain(..) {
            if self.flow_control.collecting_block {
                self.flow_control.current_block.pipelines.push(pipeline);
            } else {
                //if self.flow_control.skipping() && !is_flow_control_command(&pipeline.jobs[0].command) {
                    //continue;
                //}
                self.run_pipeline(&pipeline);
            }
        }
    }

    fn run_pipeline(&mut self, pipeline: &Pipeline) -> Option<i32> {
        let mut pipeline = self.variables.expand_pipeline(pipeline, &self.directory_stack);
        pipeline.expand_globs();
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
        // Retrieve the exit_status and set the $? variable and history.previous_status
        if let Some(code) = exit_status {
            self.variables.set_var("?", &code.to_string());
            self.history.previous_status = code;
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
                                if let Some(status) = args.get(1) {
                                    if let Ok(status) = status.parse::<i32>() {
                                        process::exit(status);
                                    }
                                }
                                process::exit(shell.history.previous_status);
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
                                shell.history.history(args)
                            },
                        });

        //commands.insert("if",
                        //Command {
                            //name: "if",
                            //help: "Conditionally execute code",
                            //main: box |args: &[String], shell: &mut Shell| -> i32 {
                                //shell.flow_control.if_(args)
                            //},
                        //});

        //commands.insert("else",
                        //Command {
                            //name: "else",
                            //help: "Execute code if a previous condition was false",
                            //main: box |args: &[String], shell: &mut Shell| -> i32 {
                                //shell.flow_control.else_(args)
                            //},
                        //});

        //commands.insert("end",
                        //Command {
                            //name: "end",
                            //help: "End a code block",
                            //main: box |args: &[String], shell: &mut Shell| -> i32 {
                                //shell.flow_control.end(args)
                            //},
                        //});

        //commands.insert("for",
                        //Command {
                            //name: "for",
                            //help: "Iterate through a list",
                            //main: box |args: &[String], shell: &mut Shell| -> i32 {
                                //shell.flow_control.for_(args)
                            //},
                        //});

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

        //commands.insert("fn",
                        //Command {
                            //name: "fn",
                            //help: "Create a function",
                            //main: box |args: &[String], shell: &mut Shell| -> i32 {
                                //shell.flow_control.fn_(args)
                            //},
                        //});

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
    Shell::default().execute();
}
