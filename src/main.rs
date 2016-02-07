#![feature(deque_extras)]
#![feature(box_syntax)]
#![feature(plugin)]
#![plugin(peg_syntax_ext)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::env;
use std::process;

use self::directory_stack::DirectoryStack;
use self::input_editor::readln;
use self::peg::{parse, Job};
use self::variables::Variables;
use self::history::History;
use self::flow_control::{FlowControl, is_flow_control_command, Statement};

pub mod directory_stack;
pub mod to_num;
pub mod input_editor;
pub mod peg;
pub mod variables;
pub mod history;
pub mod flow_control;


/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    variables: Variables,
    flow_control: FlowControl,
    directory_stack: DirectoryStack,
    history: History,
}

impl Shell {
    /// Panics if DirectoryStack construction fails
    pub fn new() -> Self {
        Shell {
            variables: Variables::new(),
            flow_control: FlowControl::new(),
            directory_stack: DirectoryStack::new().expect(""),
            history: History::new(),
        }
    }

    pub fn print_prompt(&self) {
        self.print_prompt_prefix();
        match self.flow_control.current_statement {
            Statement::For(_, _) => self.print_for_prompt(),
            Statement::Default => self.print_default_prompt(),
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

    fn print_default_prompt(&self) {
        let cwd = env::current_dir().ok().map_or("?".to_string(),
                                                 |ref p| p.to_str().unwrap_or("?").to_string());
        print!("ion:{}# ", cwd);
    }

    fn on_command(&mut self, command_string: &str, commands: &HashMap<&str, Command>) {
        self.history.add(command_string.to_string());

        let mut jobs = parse(command_string);

        // Execute commands
        for job in jobs.drain(..) {
            if self.flow_control.collecting_block {
                // TODO move this logic into "end" command
                if job.command == "end" {
                    self.flow_control.collecting_block = false;
                    let block_jobs: Vec<Job> = self.flow_control
                                                   .current_block
                                                   .jobs
                                                   .drain(..)
                                                   .collect();
                    let mut variable = String::new();
                    let mut values: Vec<String> = vec![];
                    if let Statement::For(ref var, ref vals) = self.flow_control.current_statement {
                        variable = var.clone();
                        values = vals.clone();
                    }
                    for value in values {
                        self.variables.set_var(&variable, &value);
                        for job in block_jobs.iter() {
                            self.run_job(job, commands);
                        }
                    }
                    self.flow_control.current_statement = Statement::Default;
                } else {
                    self.flow_control.current_block.jobs.push(job);
                }
            } else {
                if self.flow_control.skipping() && !is_flow_control_command(&job.command) {
                    continue;
                }
                self.run_job(&job, commands);
            }
        }
    }

    fn run_job(&mut self, job: &Job, commands: &HashMap<&str, Command>) {
        let job = self.variables.expand_job(job);
        if let Some(command) = commands.get(job.command.as_str()) {
            (*command.main)(job.args.as_slice(), self);
        } else {
            self.run_external_commmand(&job.args);
        }
    }


    fn run_external_commmand(&mut self, args: &Vec<String>) {
        if let Some(path) = args.get(0) {
            let mut command = process::Command::new(path);
            for i in 1..args.len() {
                if let Some(arg) = args.get(i) {
                    command.arg(arg);
                }
            }
            match command.spawn() {
                Ok(mut child) => {
                    match child.wait() {
                        Ok(status) => {
                            if let Some(code) = status.code() {
                                self.variables.set_var("?", &code.to_string());
                            } else {
                                println!("{}: No child exit code", path);
                            }
                        }
                        Err(err) => println!("{}: Failed to wait: {}", path, err),
                    }
                }
                Err(err) => println!("{}: Failed to execute: {}", path, err),
            }
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
///     main: box|args: &[String], &mut Shell| {
///         println!("Say 'hello' to my command! :-D");
///     }
/// }
/// ```
pub struct Command {
    pub name: &'static str,
    pub help: &'static str,
    pub main: Box<Fn(&[String], &mut Shell)>,
}

impl Command {
    /// Return the map from command names to commands
    pub fn map() -> HashMap<&'static str, Self> {
        let mut commands: HashMap<&str, Self> = HashMap::new();

        commands.insert("cd",
                        Command {
                            name: "cd",
                            help: "To change the current directory\n    cd <your_destination>",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.directory_stack.cd(args);
                            },
                        });

        commands.insert("dirs",
                        Command {
                            name: "dirs",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.directory_stack.dirs(args);
                            },
                        });

        commands.insert("exit",
                        Command {
                            name: "exit",
                            help: "To exit the curent session",
                            main: box |_: &[String], _: &mut Shell| {
                                process::exit(0);
                                // TODO exit with argument 1 as parameter
                            },
                        });

        commands.insert("let",
                        Command {
                            name: "let",
                            help: "View, set or unset variables",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.variables.let_(args);
                            },
                        });

        commands.insert("read",
                        Command {
                            name: "read",
                            help: "To read some variables\n    read <my_variable>",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.variables.read(args);
                            },
                        });

        commands.insert("pushd",
                        Command {
                            name: "pushd",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.directory_stack.pushd(args);
                            },
                        });

        commands.insert("popd",
                        Command {
                            name: "popd",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.directory_stack.popd(args);
                            },
                        });

        commands.insert("history",
                        Command {
                            name: "history",
                            help: "Display all commands previously executed",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.history.history(args);
                            },
                        });

        commands.insert("if",
                        Command {
                            name: "if",
                            help: "Conditionally execute code",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.flow_control.if_(args);
                            },
                        });

        commands.insert("else",
                        Command {
                            name: "else",
                            help: "Execute code if a previous condition was false",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.flow_control.else_(args);
                            },
                        });

        commands.insert("end",
                        Command {
                            name: "end",
                            help: "end a code block",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.flow_control.end(args);
                            },
                        });

        commands.insert("for",
                        Command {
                            name: "for",
                            help: "Loop over something",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.flow_control.for_(args);
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
                            help: "Display a little helper for a given command\n    help ls",
                            main: box move |args: &[String], _: &mut Shell| {
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
                                    for (command, _help) in command_helper.iter() {
                                        println!("{}", command);
                                    }
                                }
                            },
                        });

        commands
    }
}

fn main() {
    let commands = Command::map();
    let mut shell = Shell::new();

    for arg in env::args().skip(1) {
        let mut command_list = String::new();
        if let Ok(mut file) = File::open(&arg) {
            if let Err(message) = file.read_to_string(&mut command_list) {
                println!("{}: Failed to read {}", message, arg);
            }
        }
        shell.on_command(&command_list, &commands);
        return;
    }

    loop {
        shell.print_prompt();

        if let Some(command) = readln() {
            let command = command.trim();
            if !command.is_empty() {
                shell.on_command(command, &commands);
            }
        } else {
            break;
        }
    }
}
