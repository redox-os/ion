#![feature(box_syntax)]
#![feature(convert)]

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::env;
use std::process;

use self::to_num::ToNum;
use self::directory_stack::DirectoryStack;
use self::input_editor::readln;
use self::tokenizer::tokenize;
use self::expansion::expand_tokens;
use self::parser::parse;

pub mod builtin;
pub mod directory_stack;
pub mod to_num;
pub mod input_editor;
pub mod tokenizer;
pub mod parser;
pub mod expansion;

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    pub variables: BTreeMap<String, String>,
    pub modes: Vec<Mode>,
    pub directory_stack: DirectoryStack,
}

impl Shell {
    /// Panics if DirectoryStack construction fails
    pub fn new() -> Self {
        Shell {
            variables: BTreeMap::new(),
            modes: vec![],
            directory_stack: DirectoryStack::new().expect(""),
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
///     main: box|args: &[String]| {
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
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::cd(args);
                            },
                        });

        commands.insert("exit",
                        Command {
                            name: "exit",
                            help: "To exit the curent session",
                            main: box |_: &[String], _: &mut Shell| {},
                        });

        commands.insert("read",
                        Command {
                            name: "read",
                            help: "To read some variables\n    read <my_variable>",
                            main: box |args: &[String], shell: &mut Shell| {
                                builtin::read(args, &mut shell.variables);
                            },
                        });

        commands.insert("run",
                        Command {
                            name: "run",
                            help: "Run a script\n    run <script>",
                            main: box |args: &[String], shell: &mut Shell| {
                                builtin::run(args, &mut shell.variables);
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

        commands.insert("dirs",
                        Command {
                            name: "dirs",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], shell: &mut Shell| {
                                shell.directory_stack.dirs(args);
                            },
                        });

        // TODO: Someone should implement FromIterator for HashMap before
        //       changing the type back to HashMap
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

pub struct Mode {
    value: bool,
}

fn on_command(command_string: &str, commands: &HashMap<&str, Command>, shell: &mut Shell) {
    // Show variables
    if command_string == "$" {
        for (key, value) in shell.variables.iter() {
            println!("{}={}", key, value);
        }
        return;
    }

    let mut tokens = expand_tokens(&mut tokenize(command_string), &mut shell.variables);
    let jobs = parse(&mut tokens);

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

            shell.modes.insert(0, Mode { value: value });
            continue;
        }

        if job.command == "else" {
            if let Some(mode) = shell.modes.get_mut(0) {
                mode.value = !mode.value;
            } else {
                println!("Syntax error: else found with no previous if");
            }
            continue;
        }

        if job.command == "fi" {
            if !shell.modes.is_empty() {
                shell.modes.remove(0);
            } else {
                println!("Syntax error: fi found with no previous if");
            }
            continue;
        }

        let mut skipped: bool = false;
        for mode in shell.modes.iter() {
            if !mode.value {
                skipped = true;
                break;
            }
        }
        if skipped {
            continue;
        }

        // Set variables
        if let Some(i) = job.command.find('=') {
            let name = job.command[0..i].trim();
            let mut value = job.command[i + 1..job.command.len()].trim().to_string();

            for i in 0..job.args.len() {
                if let Some(arg) = job.args.get(i) {
                    value = value + " " + &arg;
                }
            }

            set_var(&mut shell.variables, name, &value);
            continue;
        }

        // Commands
        let mut args = job.args.clone();
        args.insert(0, job.command.clone());
        if let Some(command) = commands.get(&job.command.as_str()) {
            (*command.main)(&args, shell);
        } else {
            run_external_commmand(args, &mut shell.variables);
        }
    }
}


pub fn set_var(variables: &mut BTreeMap<String, String>, name: &str, value: &str) {
    if name.is_empty() {
        return;
    }

    if value.is_empty() {
        variables.remove(&name.to_string());
    } else {
        variables.insert(name.to_string(), value.to_string());
    }
}

fn print_prompt(modes: &Vec<Mode>) {
    for mode in modes.iter().rev() {
        if mode.value {
            print!("+ ");
        } else {
            print!("- ");
        }
    }

    let cwd = match env::current_dir() {
        Ok(path) => {
            match path.to_str() {
                Some(path_str) => path_str.to_string(),
                None => "?".to_string(),
            }
        }
        Err(_) => "?".to_string(),
    };

    print!("ion:{}# ", cwd);
    if let Err(message) = stdout().flush() {
        println!("{}: failed to flush prompt to stdout", message);
    }
}

fn run_external_commmand(args: Vec<String>, variables: &mut BTreeMap<String, String>) {
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
                            set_var(variables, "?", &code.to_string());
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
        on_command(&command_list, &commands, &mut shell);

        return;
    }

    loop {

        print_prompt(&shell.modes);

        if let Some(command_original) = readln() {
            let command = command_original.trim();
            if command == "exit" {
                break;
            } else if !command.is_empty() {
                on_command(&command, &commands, &mut shell);
            }
        } else {
            break;
        }
    }
}
