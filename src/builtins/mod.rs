pub mod source;
pub mod variables;
pub mod functions;
pub mod calc;

mod conditionals;
mod job_control;
mod test;
mod echo;
mod set;
mod exists;
mod ion;

use self::conditionals::{contains, ends_with, starts_with};
use self::echo::echo;
use self::exists::exists;
use self::functions::fn_;
use self::ion::ion_docs;
use self::source::source;
use self::test::test;
use self::variables::{alias, drop_alias, drop_array, drop_variable};

use std::error::Error;
use std::env;
use std::io::{self, Write};
use std::path::Path;

use parser::QuoteTerminator;
use shell::{self, FlowLogic, Shell, ShellHistory};
use shell::job_control::{JobControl, ProcessState};
use shell::status::*;
use sys;

macro_rules! map {
    ($($name:expr => $func:ident: $help:expr),+) => {{
        BuiltinMap {
            name: &[$($name),+],
            help: &[$($help),+],
            functions: &[$($func),+],
        }
    }
}}

pub const BUILTINS: &'static BuiltinMap = &map!(
    "echo" => builtin_echo : "Display a line of text",
    "cd" => builtin_cd : "Change the current directory\n    cd <path>",
    "dirs" => builtin_dirs : "Display the current directory stack",
    "pushd" => builtin_pushd : "Push a directory to the stack",
    "popd" => builtin_popd : "Pop a directory from the stack",
    "alias" => builtin_alias : "View, set or unset aliases",
    "unalias" => builtin_unalias : "Delete an alias",
    "fn" => builtin_fn : "Print list of functions",
    "read" => builtin_read : "Read some variables\n    read <variable>",
    "drop" => builtin_drop : "Delete a variable",
    "matches" => builtin_matches : "Checks if a string matches a given regex",
    "not" => builtin_not : "Reverses the exit status value of the given command.",
    "set" => builtin_set : "Set or unset values of shell options and positional parameters.",
    "eval" => builtin_eval : "evaluates the evaluated expression",
    "exit" => builtin_exit : "Exits the current session",
    "wait" => builtin_wait : "Waits until all running background processes have completed",
    "jobs" => builtin_jobs : "Displays all jobs that are attached to the background",
    "bg" => builtin_bg : "Resumes a stopped background process",
    "fg" => builtin_fg : "Resumes and sets a background process as the active process",
    "suspend" => builtin_suspend : "Suspends the shell with a SIGTSTOP signal",
    "disown" => builtin_disown : "Disowning a process removes that process from the shell's \
        background process table.",
    "history" => builtin_history : "Display a log of all commands previously executed",
    "source" => builtin_source : "Evaluate the file following the command or re-initialize the \
        init file",
    "test" => builtin_test : "Performs tests on files and text",
    "calc" => builtin_calc : "Calculate a mathematical expression",
    "true" => builtin_true : "Do nothing, successfully",
    "false" => builtin_false : "Do nothing, unsuccessfully",
    "help" => builtin_help : "Display helpful information about a given command or list commands \
        if none specified\n    help <command>",
    "and" => builtin_and : "Execute the command if the shell's previous status is success",
    "or" => builtin_or : "Execute the command if the shell's previous status is failure",
    "starts-with" => starts_with : "Evaluates if the supplied argument starts with a given string",
    "ends-with" => ends_with :"Evaluates if the supplied argument ends with a given string",
    "contains" => contains : "Evaluates if the supplied argument contains a given string",
    "exists" => builtin_exists : "Performs tests on files and text",
    "which" => builtin_which : "Shows the full path of commands",
    "ion-docs" => ion_docs : "Opens the Ion manual"
);

/// Structure which represents a Terminal's command.
/// This command structure contains a name, and the code which run the
/// functionnality associated to this one, with zero, one or several argument(s).
pub struct Builtin {
    pub name: &'static str,
    pub help: &'static str,
    pub main: fn(&[&str], &mut Shell) -> i32,
}

pub struct BuiltinMap {
    pub(crate) name:      &'static [&'static str],
    pub(crate) help:      &'static [&'static str],
    pub(crate) functions: &'static [fn(&[&str], &mut Shell) -> i32],
}

impl BuiltinMap {
    pub fn get(&self, func: &str) -> Option<Builtin> {
        self.name.iter().position(|&name| name == func).map(|pos| unsafe {
            Builtin {
                name: *self.name.get_unchecked(pos),
                help: *self.help.get_unchecked(pos),
                main: *self.functions.get_unchecked(pos),
            }
        })
    }

    pub fn keys(&self) -> &'static [&'static str] { self.name }

    pub fn contains_key(&self, func: &str) -> bool { self.name.iter().any(|&name| name == func) }
}

// Definitions of simple builtins go here

fn builtin_cd(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.directory_stack.cd(args, &shell.variables) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.as_bytes());
            FAILURE
        }
    }
}

fn builtin_dirs(args: &[&str], shell: &mut Shell) -> i32 { shell.directory_stack.dirs(args) }

fn builtin_pushd(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.directory_stack.pushd(args, &shell.variables) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.as_bytes());
            FAILURE
        }
    }
}

fn builtin_popd(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.directory_stack.popd(args) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.as_bytes());
            FAILURE
        }
    }
}

fn builtin_alias(args: &[&str], shell: &mut Shell) -> i32 {
    let args_str = args[1..].join(" ");
    alias(&mut shell.variables, &args_str)
}

fn builtin_unalias(args: &[&str], shell: &mut Shell) -> i32 {
    drop_alias(&mut shell.variables, args)
}

fn builtin_fn(_: &[&str], shell: &mut Shell) -> i32 { fn_(&mut shell.functions) }

fn builtin_read(args: &[&str], shell: &mut Shell) -> i32 { shell.variables.read(args) }

fn builtin_drop(args: &[&str], shell: &mut Shell) -> i32 {
    if args.len() >= 2 && args[1] == "-a" {
        drop_array(&mut shell.variables, args)
    } else {
        drop_variable(&mut shell.variables, args)
    }
}

fn builtin_not(args: &[&str], shell: &mut Shell) -> i32 {
    let cmd = args[1..].join(" ");
    shell.on_command(&cmd);
    match shell.previous_status {
        SUCCESS => FAILURE,
        FAILURE => SUCCESS,
        _ => shell.previous_status,
    }
}

fn builtin_set(args: &[&str], shell: &mut Shell) -> i32 { set::set(args, shell) }
fn builtin_eval(args: &[&str], shell: &mut Shell) -> i32 {
    let evaluated_command = args[1..].join(" ");
    let mut buffer = QuoteTerminator::new(evaluated_command);
    if buffer.check_termination() {
        shell.on_command(&buffer.consume());
        shell.previous_status
    } else {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        let _ = writeln!(stderr, "ion: supplied eval expression was not terminted");
        FAILURE
    }
}
fn builtin_history(args: &[&str], shell: &mut Shell) -> i32 { shell.print_history(args) }

fn builtin_source(args: &[&str], shell: &mut Shell) -> i32 {
    match source(shell, args) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.as_bytes());
            FAILURE
        }
    }
}

fn builtin_echo(args: &[&str], _: &mut Shell) -> i32 {
    match echo(args) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.description().as_bytes());
            FAILURE
        }
    }
}

fn builtin_test(args: &[&str], _: &mut Shell) -> i32 {
    match test(args) {
        Ok(true) => SUCCESS,
        Ok(false) => FAILURE,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "{}", why);
            FAILURE
        }
    }
}

fn builtin_calc(args: &[&str], _: &mut Shell) -> i32 {
    match calc::calc(&args[1..]) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "{}", why);
            FAILURE
        }
    }
}

fn builtin_true(_: &[&str], _: &mut Shell) -> i32 { SUCCESS }

fn builtin_false(_: &[&str], _: &mut Shell) -> i32 { FAILURE }

fn builtin_wait(_: &[&str], shell: &mut Shell) -> i32 {
    shell.wait_for_background();
    SUCCESS
}

fn builtin_jobs(_: &[&str], shell: &mut Shell) -> i32 {
    job_control::jobs(shell);
    SUCCESS
}

fn builtin_bg(args: &[&str], shell: &mut Shell) -> i32 { job_control::bg(shell, &args[1..]) }

fn builtin_fg(args: &[&str], shell: &mut Shell) -> i32 { job_control::fg(shell, &args[1..]) }

fn builtin_suspend(_: &[&str], _: &mut Shell) -> i32 {
    shell::signals::suspend(0);
    SUCCESS
}

fn builtin_disown(args: &[&str], shell: &mut Shell) -> i32 {
    job_control::disown(shell, &args[1..])
}

fn builtin_help(args: &[&str], shell: &mut Shell) -> i32 {
    let builtins = shell.builtins;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if let Some(command) = args.get(1) {
        if builtins.contains_key(command) {
            if let Some(bltin) = builtins.get(command) {
                let _ = stdout.write_all(bltin.help.as_bytes());
                let _ = stdout.write_all(b"\n");
            }
        } else {
            let _ = stdout.write_all(b"Command helper not found [run 'help']...");
            let _ = stdout.write_all(b"\n");
        }
    } else {
        let mut commands = builtins.keys();

        let mut buffer: Vec<u8> = Vec::new();
        for command in commands {
            let _ = writeln!(buffer, "{}", command);
        }
        let _ = stdout.write_all(&buffer);
    }
    SUCCESS
}

fn builtin_exit(args: &[&str], shell: &mut Shell) -> i32 {
    // Kill all active background tasks before exiting the shell.
    for process in shell.background.lock().unwrap().iter() {
        if process.state != ProcessState::Empty {
            let _ = sys::kill(process.pid, sys::SIGTERM);
        }
    }
    let previous_status = shell.previous_status;
    shell.exit(args.get(1).and_then(|status| status.parse::<i32>().ok()).unwrap_or(previous_status))
}

use regex::Regex;
fn builtin_matches(args: &[&str], _: &mut Shell) -> i32 {
    if args[1..].len() != 2 {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        let _ = stderr.write_all(b"match takes two arguments\n");
        return BAD_ARG;
    }
    let input = args[1];
    let re = match Regex::new(args[2]) {
        Ok(r) => r,
        Err(e) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr
                .write_all(format!("couldn't compile input regex {}: {}\n", args[2], e).as_bytes());
            return FAILURE;
        }
    };

    if re.is_match(input) {
        SUCCESS
    } else {
        FAILURE
    }
}

fn builtin_and(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.previous_status {
        SUCCESS => {
            let cmd = args[1..].join(" ");
            shell.on_command(&cmd);
            shell.previous_status
        }
        _ => shell.previous_status,
    }
}

fn builtin_or(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.previous_status {
        FAILURE => {
            let cmd = args[1..].join(" ");
            shell.on_command(&cmd);
            shell.previous_status
        }
        _ => shell.previous_status,
    }
}

fn builtin_exists(args: &[&str], shell: &mut Shell) -> i32 {
    match exists(args, shell) {
        Ok(true) => SUCCESS,
        Ok(false) => FAILURE,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "{}", why);
            FAILURE
        }
    }
}

fn builtin_which(args: &[&str], shell: &mut Shell) -> i32 {
    if args[1..].len() != 1 {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        let _ = stderr.write_all(b"which takes one argument\n");
        return BAD_ARG;
    }

    let command = args[1];

    if let Some(alias) = shell.variables.aliases.get(command) {
        println!("{}: alias to {}", command, alias);
        SUCCESS
    } else if shell.builtins.contains_key(command) {
        println!("{}: built-in shell command", command);
        SUCCESS
    } else if shell.functions.contains_key(command) {
        println!("{}: function", command);
        SUCCESS
    } else {
        for path in env::var("PATH").unwrap_or("/bin".to_string())
                                    .split(sys::PATH_SEPARATOR) {
            let executable = Path::new(path).join(command);
            if executable.is_file() {
                println!("{}", executable.display());
                return SUCCESS;
            }

        }

        println!("{} not found", command);
        FAILURE
    }
}
