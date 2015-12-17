use std::collections::BTreeMap;
use std::string::String;
use std::vec::Vec;
use std::boxed::Box;
use std::fs::{self, File};
use std::io::{stdout, Read, Write};
use std::env;
use std::process;
use std::thread;

use self::to_num::ToNum;
use self::input_editor::readln;
use self::tokenizer::{Token, tokenize};
use self::expansion::expand_tokens;
use self::parser::{parse, Job};

pub mod to_num;
pub mod input_editor;
pub mod tokenizer;
pub mod parser;
pub mod expansion;

/// Structure which represents a Terminal's command.
/// This command structure contains a name, and the code which run the functionnality associated to this one, with zero, one or several argument(s).
/// # Example
/// ```
/// let my_command = Command {
///     name: "my_command",
///     help: "Describe what my_command does followed by a newline showing usage",
///     main: box|args: &Vec<String>| {
///         println!("Say 'hello' to my command! :-D");
///     }
/// }
/// ```
pub struct Command {
    pub name: &'static str,
    pub help: &'static str,
    pub main: Box<Fn(&Vec<String>, &mut BTreeMap<String, String>, &mut Vec<Mode>)>,
}

impl Command {
    /// Return the map from command names to commands
    pub fn map() -> BTreeMap<String, Self> {
        let mut commands: BTreeMap<String, Self> = BTreeMap::new();

        commands.insert("cat".to_string(), Command {
            name: "cat",
            help: "To display a file in the output\n    cat <your_file>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                let path = args.get(1).map_or(String::new(), |arg| arg.clone());

                match File::open(&path) {
                    Ok(mut file) => {
                        let mut string = String::new();
                        match file.read_to_string(&mut string) {
                            Ok(_) => println!("{}", string),
                            Err(err) => println!("Failed to read: {}: {}", path, err),
                        }
                    },
                    Err(err) => println!("Failed to open file: {}: {}", path, err)
                }
            }),
        });

        commands.insert("cd".to_string(), Command {
            name: "cd",
            help: "To change the current directory\n    cd <your_destination>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match args.get(1) {
                    Some(path) => {
                        if let Err(err) = env::set_current_dir(&path) {
                            println!("Failed to set current dir to {}: {}", path, err);
                        }
                    }
                    None => println!("No path given"),
                }
            }),
        });

        commands.insert("echo".to_string(), Command {
            name: "echo",
            help: "To display some text in the output\n    echo Hello world!",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                let echo = args.iter()
                               .skip(1)
                               .fold(String::new(), |string, arg| string + " " + arg);
                println!("{}", echo.trim());
            }),
        });

        commands.insert("else".to_string(), Command {
            name: "else",
            help: "",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {}),
        });

        commands.insert("exec".to_string(), Command {
            name: "exec",
            help: "To execute a binary in the output\n    exec <my_binary>",
            main: Box::new(|args: &Vec<String>, variables: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                if let Some(path) = args.get(1) {
                    let mut command = process::Command::new(path);
                    for i in 2 .. args.len() {
                        if let Some(arg) = args.get(i){
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
                                },
                                Err(err) => println!("{}: Failed to wait: {}", path, err)
                            }
                        },
                        Err(err) => println!("{}: Failed to execute: {}", path, err)
                    }
                }
            }),
        });

        commands.insert("exit".to_string(), Command {
            name: "exit",
            help: "To exit the curent session",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {}),
        });

        commands.insert("fi".to_string(), Command {
            name: "fi",
            help: "",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {}),
        });

        commands.insert("free".to_string(), Command {
            name: "free",
            help: "Show memory information\n    free",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match File::open("memory:") {
                    Ok(mut file) => {
                        let mut string = String::new();
                        match file.read_to_string(&mut string) {
                            Ok(_) => println!("{}", string),
                            Err(err) => println!("Failed to read: memory: {}", err),
                        }
                    }
                    Err(err) => println!("Failed to open file: memory: {}", err)
                }
            }),
        });

        commands.insert("if".to_string(), Command {
            name: "if",
            help: "",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {}),
        });

        commands.insert("ls".to_string(), Command {
            name: "ls",
            help: "To list the content of the current directory\n    ls",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                let path = args.get(1).map_or(".".to_string(), |arg| arg.clone());

                let mut entries = Vec::new();
                match fs::read_dir(&path) {
                    Ok(dir) => {
                        for entry_result in dir {
                            match entry_result {
                                Ok(entry) => {
                                    let directory = match entry.file_type() {
                                        Ok(file_type) => file_type.is_dir(),
                                        Err(err) => {
                                            println!("Failed to read file type: {}", err);
                                            false
                                        }
                                    };

                                    match entry.file_name().to_str() {
                                        Some(path_str) => {
                                            if directory {
                                                entries.push(path_str.to_string() + "/")
                                            } else {
                                                entries.push(path_str.to_string())
                                            }
                                        },
                                        None => println!("Failed to convert path to string")
                                    }
                                },
                                Err(err) => println!("Failed to read entry: {}", err)
                            }
                        }
                    },
                    Err(err) => println!("Failed to open directory: {}: {}", path, err)
                }

                entries.sort();

                for entry in entries {
                    println!("{}", entry);
                }
            }),
        });

        commands.insert("mkdir".to_string(), Command {
            name: "mkdir",
            help: "To create a directory in the current directory\n    mkdir <my_new_directory>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match args.get(1) {
                    Some(dir_name) => if let Err(err) = fs::create_dir(dir_name) {
                        println!("Failed to create: {}: {}", dir_name, err);
                    },
                    None => println!("No name provided"),
                }
            }),
        });

        commands.insert("ps".to_string(), Command {
            name: "ps",
            help: "Show process list\n    ps",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match File::open("context:") {
                    Ok(mut file) => {
                        let mut string = String::new();
                        match file.read_to_string(&mut string) {
                            Ok(_) => println!("{}", string),
                            Err(err) => println!("Failed to read: context: {}", err),
                        }
                    }
                    Err(err) => println!("Failed to open file: context: {}", err)
                }
            }),
        });

        commands.insert("pwd".to_string(), Command {
            name: "pwd",
            help: "To output the path of the current directory\n    pwd",
            main: Box::new(|_: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match env::current_dir() {
                    Ok(path) => match path.to_str() {
                        Some(path_str) => println!("{}", path_str),
                        None => println!("?")
                    },
                    Err(err) => println!("Failed to get current dir: {}", err)
                }
            }),
        });

        commands.insert("read".to_string(), Command {
            name: "read",
            help: "To read some variables\n    read <my_variable>",
            main: Box::new(|args: &Vec<String>, variables: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                for i in 1..args.len() {
                    if let Some(arg_original) = args.get(i) {
                        let arg = arg_original.trim();
                        print!("{}=", arg);
                        stdout().flush();
                        if let Some(value_original) = readln() {
                            let value = value_original.trim();
                            set_var(variables, arg, value);
                        }
                    }
                }
            }),
        });

        commands.insert("rm".to_string(), Command {
            name: "rm",
            help: "Remove a file\n    rm <file>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match args.get(1) {
                    Some(path) => if fs::remove_file(path).is_err() {
                        println!("Failed to remove: {}", path);
                    },
                    None => println!("No name provided"),
                }
            }),
        });

        commands.insert("rmdir".to_string(), Command {
            name: "rmdir",
            help: "Remove a directory\n    rmdir <directory>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match args.get(1) {
                    Some(path) => if fs::remove_dir(path).is_err() {
                        println!("Failed to remove: {}", path);
                    },
                    None => println!("No name provided"),
                }
            }),
        });

        commands.insert("run".to_string(), Command {
            name: "run",
            help: "Run a script\n    run <script>",
            main: Box::new(|args: &Vec<String>, variables: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                let path = "/apps/shell/main.bin";

                let mut command = process::Command::new(path);
                for i in 1 .. args.len() {
                    if let Some(arg) = args.get(i){
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
                            },
                            Err(err) => println!("{}: Failed to wait: {}", path, err)
                        }
                    },
                    Err(err) => println!("{}: Failed to execute: {}", path, err)
                }
            })
        });

        commands.insert("sleep".to_string(), Command {
            name: "sleep",
            help: "Make a sleep in the current session\n    sleep <number_of_seconds>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                let secs = args.get(1).map_or(0, |arg| arg.to_num());
                thread::sleep_ms(secs as u32 * 1000);
            }),
        });

        // Simple command to create a file, in the current directory
        // The file has got the name given as the first argument of the command
        // If the command have no arguments, the command don't create the file
        commands.insert("touch".to_string(), Command {
            name: "touch",
            help: "To create a file, in the current directory\n    touch <my_file>",
            main: Box::new(|args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                match args.get(1) {
                    Some(file_name) => if let Err(err) = File::create(file_name) {
                        println!("Failed to create: {}: {}", file_name, err);
                    },
                    None => println!("No name provided"),
                }
            }),
        });

        // TODO: Someone should implement FromIterator for HashMap before
        //       changing the type back to HashMap
        let command_helper: BTreeMap<String, String> = commands
            .iter()
            .map(|(k, v)| (k.to_string(), v.help.to_string()))
            .collect();

        commands.insert("help".to_string(), Command {
            name: "help",
            help: "Display a little helper for a given command\n    help ls",
            main: Box::new(move |args: &Vec<String>, _: &mut BTreeMap<String, String>, _: &mut Vec<Mode>| {
                if let Some(command) = args.get(1) {
                    if command_helper.contains_key(command) {
                        match command_helper.get(command) {
                            Some(help) => println!("{}", help),
                            None => println!("Command helper not found [run 'help']..."),
                        }
                    } else {
                        println!("Command helper not found [run 'help']...");
                    }
                } else {
                    for (command, _help) in command_helper.iter() {
                        println!("{}", command);
                    }
                }
            }),
        });

        commands
    }
}

pub struct Mode {
    value: bool,
}

fn on_command(command_string: &str,
              commands: &BTreeMap<String, Command>,
              variables: &mut BTreeMap<String, String>,
              modes: &mut Vec<Mode>) {
    // Show variables
    if command_string == "$" {
        for (key, value) in variables.iter() {
            println!("{}={}", key, value);
        }
        return;
    }

    let mut tokens: Vec<Token> = expand_tokens(&mut tokenize(command_string), variables);
    let mut jobs: Vec<Job> = parse(&mut tokens);

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

            modes.insert(0, Mode { value: value });
            continue;
        }

        if job.command == "else" {
            let mut syntax_error = false;
            match modes.get_mut(0) {
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
            if !modes.is_empty() {
                modes.remove(0);
            } else {
                syntax_error = true;
            }
            if syntax_error {
                println!("Syntax error: fi found with no previous if");
            }
            continue;
        }

        let mut skipped: bool = false;
        for mode in modes.iter() {
            if !mode.value {
                skipped = true;
                break
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

            set_var(variables, name, &value);
            continue;
        }

        // Commands
        if let Some(command) = commands.get(&job.command) {
            let mut args = job.args.clone();
            args.insert(0, job.command.clone());
            (*command.main)(&args, variables, modes);
        }
        else {
            println!("Unknown command: '{}'", job.command);
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
            Ok(path) => match path.to_str() {
                Some(path_str) => path_str.to_string(),
                None => "?".to_string()
            },
            Err(_) => "?".to_string()
        };

        print!("ion:{}# ", cwd);
        stdout().flush();
}

fn real_main() {
    let commands = Command::map();
    let mut variables: BTreeMap<String, String> = BTreeMap::new();
    let mut modes: Vec<Mode> = vec![];

    for arg in env::args().skip(1) {
        let mut command_list = String::new();
        if let Ok(mut file) = File::open(arg) {
            file.read_to_string(&mut command_list);
        }
        on_command(&command_list, &commands, &mut variables, &mut modes);

        return;
    }

    loop {

        print_prompt(&modes);

        if let Some(command_original) = readln() {
            let command = command_original.trim();
            if command == "exit" {
                break;
            } else if !command.is_empty() {
                on_command(&command, &commands, &mut variables, &mut modes);
            }
        } else {
            break;
        }
    }
}

#[cfg(target_os = "redox")]
#[no_mangle]
pub fn main(){
    real_main();
}

#[cfg(not(target_os = "redox"))]
fn main(){
    real_main();
}
