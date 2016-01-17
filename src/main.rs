#![feature(box_syntax)]

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::env;
use std::process;

use self::to_num::ToNum;
use self::input_editor::readln;
use self::tokenizer::{Token, tokenize};
use self::expansion::expand_tokens;
use self::parser::{parse, Job};

pub mod builtin;
pub mod to_num;
pub mod input_editor;
pub mod tokenizer;
pub mod parser;
pub mod expansion;

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

/// This struct will contain all of the data structures related to this
/// instance of the shell.
pub struct Shell {
    pub variables: BTreeMap<String, String>,
    pub modes: Vec<Mode>,
}

impl Command {
    /// Return the map from command names to commands
    pub fn map() -> BTreeMap<String, Self> {
        let mut commands: BTreeMap<String, Self> = BTreeMap::new();

        commands.insert("cat".to_string(),
                        Command {
                            name: "cat",
                            help: "To display a file in the output\n    cat <your_file>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::cat(args);
                            },
                        });

        commands.insert("cd".to_string(),
                        Command {
                            name: "cd",
                            help: "To change the current directory\n    cd <your_destination>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::cd(args);
                            },
                        });

        commands.insert("echo".to_string(),
                        Command {
                            name: "echo",
                            help: "To display some text in the output\n    echo Hello world!",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::echo(args);
                            },
                        });

        commands.insert("exit".to_string(),
                        Command {
                            name: "exit",
                            help: "To exit the curent session",
                            main: box |_: &[String], _: &mut Shell| {},
                        });

        commands.insert("free".to_string(),
                        Command {
                            name: "free",
                            help: "Show memory information\n    free",
                            main: box |_: &[String], _: &mut Shell| {
                                builtin::free();
                            },
                        });

        commands.insert("ls".to_string(),
                        Command {
                            name: "ls",
                            help: "To list the content of the current directory\n    ls",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::ls(args);
                            },
                        });

        commands.insert("mkdir".to_string(),
                        Command {
                            name: "mkdir",
                            help: "To create a directory in the current directory\n    mkdir \
                                   <my_new_directory>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::mkdir(args);
                            },
                        });

        commands.insert("poweroff".to_string(),
                        Command {
                            name: "poweroff",
                            help: "poweroff utility has the machine remove power, if \
                                   possible\n\tpoweroff",
                            main: box |_: &[String], _: &mut Shell| {
                                builtin::poweroff();
                            },
                        });

        commands.insert("ps".to_string(),
                        Command {
                            name: "ps",
                            help: "Show process list\n    ps",
                            main: box |_: &[String], _: &mut Shell| {
                                builtin::ps();
                            },
                        });

        commands.insert("pwd".to_string(),
                        Command {
                            name: "pwd",
                            help: "To output the path of the current directory\n    pwd",
                            main: box |_: &[String], _: &mut Shell| {
                                builtin::pwd();
                            },
                        });

        commands.insert("read".to_string(),
                        Command {
                            name: "read",
                            help: "To read some variables\n    read <my_variable>",
                            main: box |args: &[String], shell: &mut Shell| {
                                builtin::read(args, &mut shell.variables);
                            },
                        });

        commands.insert("rm".to_string(),
                        Command {
                            name: "rm",
                            help: "Remove a file\n    rm <file>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::rm(args);
                            },
                        });

        commands.insert("rmdir".to_string(),
                        Command {
                            name: "rmdir",
                            help: "Remove a directory\n    rmdir <directory>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::rmdir(args);
                            },
                        });

        commands.insert("run".to_string(),
                        Command {
                            name: "run",
                            help: "Run a script\n    run <script>",
                            main: box |args: &[String], shell: &mut Shell| {
                                builtin::run(args, &mut shell.variables);
                            },
                        });

        commands.insert("sleep".to_string(),
                        Command {
                            name: "sleep",
                            help: "Make a sleep in the current session\n    sleep \
                                   <number_of_seconds>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::sleep(args);
                            },
                        });

        // Simple command to create a file, in the current directory
        // The file has got the name given as the first argument of the command
        // If the command have no arguments, the command don't create the file
        commands.insert("touch".to_string(),
                        Command {
                            name: "touch",
                            help: "To create a file, in the current directory\n    touch <my_file>",
                            main: box |args: &[String], _: &mut Shell| {
                                builtin::touch(args);
                            },
                        });

        // TODO: Someone should implement FromIterator for HashMap before
        //       changing the type back to HashMap
        let command_helper: BTreeMap<String, String> = commands.iter()
                                                               .map(|(k, v)| {
                                                                   (k.to_string(),
                                                                    v.help.to_string())
                                                               })
                                                               .collect();

        commands.insert("help".to_string(),
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

fn on_command(command_string: &str, commands: &BTreeMap<String, Command>, shell: &mut Shell) {
    // Show variables
    if command_string == "$" {
        for (key, value) in shell.variables.iter() {
            println!("{}={}", key, value);
        }
        return;
    }

    let mut tokens: Vec<Token> = expand_tokens(&mut tokenize(command_string), &mut shell.variables);
    let jobs: Vec<Job> = parse(&mut tokens);

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
            let mut syntax_error = false;
            match shell.modes.get_mut(0) {
                Some(mode) => mode.value = !mode.value,
                None => syntax_error = true,
            }
            if syntax_error {
                println!("Syntax error: else found with no previous if");
            }
            continue;
        }

        if job.command == "fi" {
            let mut syntax_error = false;
            if !shell.modes.is_empty() {
                shell.modes.remove(0);
            } else {
                syntax_error = true;
            }
            if syntax_error {
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
        if let Some(command) = commands.get(&job.command) {
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
                            set_var(variables, "?", &format!("{}", code));
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
    let mut shell = Shell {
        variables: BTreeMap::new(),
        modes: vec![],
    };

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
