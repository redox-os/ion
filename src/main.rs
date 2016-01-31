#![feature(deque_extras)]
#![feature(box_syntax)]
#![feature(plugin)]
#![plugin(peg_syntax_ext)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::env;
use std::process;

use self::to_num::ToNum;
use self::directory_stack::DirectoryStack;
use self::input_editor::readln;
use self::peg::parse;
use self::variables::Variables;
use self::history::History;

pub mod builtin;
pub mod directory_stack;
pub mod to_num;
pub mod input_editor;
pub mod peg;
pub mod variables;
pub mod history;

pub struct Mode {
    value: bool,
}

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    variables: Variables,
    modes: Vec<Mode>,
    directory_stack: DirectoryStack,
    history: History,
}

impl Shell {
    /// Panics if DirectoryStack construction fails
    pub fn new() -> Self {
        Shell {
            variables: Variables::new(),
            modes: vec![],
            directory_stack: DirectoryStack::new().expect(""),
            history: History::new(),
        }
    }

    pub fn print_prompt(&self) {
        let prompt_prefix = self.modes.iter().rev().fold(String::new(), |acc, mode| {
            acc +
            if mode.value {
                "+ "
            } else {
                "- "
            }
        });
        print!("{}", prompt_prefix);

        let cwd = env::current_dir().ok().map_or("?".to_string(),
                                                 |ref p| p.to_str().unwrap_or("?").to_string());

        print!("ion:{}# ", cwd);
        if let Err(message) = stdout().flush() {
            println!("{}: failed to flush prompt to stdout", message);
        }
    }

    fn on_command(&mut self, command_string: &str, commands: &HashMap<&str, Command>) {
        self.history.add(command_string.to_string());

        let mut jobs = parse(command_string);
        self.variables.expand_variables(&mut jobs);

        // Execute commands
        for job in jobs.iter() {
            if job.command == "if" {
                let mut value = false;

                if let Some(left) = job.args.get(0) {
                    if let Some(cmp) = job.args.get(1) {
                        if let Some(right) = job.args.get(2) {
                            if cmp == "==" {
                                value = *left == *right;
                            } else if cmp == "!=" {
                                value = *left != *right;
                            } else if cmp == ">" {
                                value = left.to_num_signed() > right.to_num_signed();
                            } else if cmp == ">=" {
                                value = left.to_num_signed() >= right.to_num_signed();
                            } else if cmp == "<" {
                                value = left.to_num_signed() < right.to_num_signed();
                            } else if cmp == "<=" {
                                value = left.to_num_signed() <= right.to_num_signed();
                            } else {
                                println!("Unknown comparison: {}", cmp);
                            }
                        } else {
                            println!("No right hand side");
                        }
                    } else {
                        println!("No comparison operator");
                    }
                } else {
                    println!("No left hand side");
                }

                self.modes.insert(0, Mode { value: value });
                continue;
            }

            if job.command == "else" {
                if let Some(mode) = self.modes.get_mut(0) {
                    mode.value = !mode.value;
                } else {
                    println!("Syntax error: else found with no previous if");
                }
                continue;
            }

            if job.command == "fi" {
                if !self.modes.is_empty() {
                    self.modes.remove(0);
                } else {
                    println!("Syntax error: fi found with no previous if");
                }
                continue;
            }

            let skipped = self.modes.iter().any(|mode| !mode.value);
            if skipped {
                continue;
            }

            // Commands
            let mut args = job.args.clone();
            args.insert(0, job.command.clone());
            if let Some(command) = commands.get(&job.command.as_str()) {
                (*command.main)(&args, self);
            } else {
                self.run_external_commmand(args);
            }
        }
    }

    fn run_external_commmand(&mut self, args: Vec<String>) {
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
                                let args = args.iter().map(|s|s.as_str());
                                shell.directory_stack.cd(args);
                            },
                        });

        commands.insert("dirs",
                        Command {
                            name: "dirs",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                let args = args.iter().map(|s|s.as_str());
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
                                let args = args.iter().map(|s|s.as_str());
                                shell.variables.let_(args);
                            },
                        });

        commands.insert("read",
                        Command {
                            name: "read",
                            help: "To read some variables\n    read <my_variable>",
                            main: box |args: &[String], shell: &mut Shell| {
                                let args = args.iter().map(|s|s.as_str());
                                shell.variables.read(args);
                            },
                        });

        commands.insert("run",
                        Command {
                            name: "run",
                            help: "Run a script\n    run <script>",
                            main: box |args: &[String], shell: &mut Shell| {
                                let args = args.iter().map(|s|s.as_str());
                                builtin::run(args, shell);
                            },
                        });

        commands.insert("pushd",
                        Command {
                            name: "pushd",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                let args = args.iter().map(|s|s.as_str());
                                shell.directory_stack.pushd(args);
                            },
                        });

        commands.insert("popd",
                        Command {
                            name: "popd",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                let args = args.iter().map(|s|s.as_str());
                                shell.directory_stack.popd(args);
                            },
                        });

        commands.insert("history",
                        Command {
                            name: "history",
                            help: "Display all commands previously executed",
                            main: box |args: &[String], shell: &mut Shell| {
                                let args = args.iter().map(|s|s.as_str());
                                shell.history.history(args);
                            },
                        });

        let command_helper: HashMap<String, String> = commands.iter()
                                                              .map(|(k, v)| {
                                                                  (k.to_string(),
                                                                   v.help.to_string())
                                                              })
                                                              .collect();

        commands.insert("help",
                        Command {
                            name: "help",
                            help: "Display a little helper for a given command\n    help ls",
                            main: box move |args: &[String], _: &mut Shell| {
                                if let Some(command) = args.get(1) {
                                    if command_helper.contains_key(command) {
                                        match command_helper.get(command) {
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
