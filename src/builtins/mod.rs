pub mod source;
pub mod variables;
pub mod functions;
pub mod calc;
pub mod random;

mod conditionals;
mod job_control;
mod man_pages;
mod test;
mod echo;
mod set;
mod status;
mod exists;
mod ion;
mod is;

use self::conditionals::{contains, ends_with, starts_with};
use self::echo::echo;
use self::exists::exists;
use self::functions::fn_;
use self::ion::ion_docs;
use self::is::is;
use self::man_pages::*;
use self::source::source;
use self::status::status;
use self::test::test;
use self::variables::{alias, drop_alias, drop_array, drop_variable};
use types::Array;

use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::path::Path;

use parser::Terminator;
use parser::pipelines::{PipeItem, Pipeline};
use shell::{self, FlowLogic, Job, JobKind, Shell, ShellHistory};
use shell::job_control::{JobControl, ProcessState};
use shell::status::*;
use sys;

const HELP_DESC: &str = "Display helpful information about a given command or list commands if \
                         none specified\n    help <command>";

const SOURCE_DESC: &str = "Evaluate the file following the command or re-initialize the init file";

const DISOWN_DESC: &str =
    "Disowning a process removes that process from the shell's background process table.";

pub type BuiltinFunction = fn(&[&str], &mut Shell) -> i32;

macro_rules! map {
    ($($name:expr => $func:ident: $help:expr),+) => {{
        BuiltinMap {
            name: &[$($name),+],
            help: &[$($help),+],
            functions: &[$($func),+],
        }
    }
}}

/// Builtins are in A-Z order.
pub const BUILTINS: &'static BuiltinMap = &map!(
    "alias" => builtin_alias : "View, set or unset aliases",
    "and" => builtin_and : "Execute the command if the shell's previous status is success",
    "bg" => builtin_bg : "Resumes a stopped background process",
    "bool" => builtin_bool : "If the value is '1' or 'true', return 0 exit status",
    "calc" => builtin_calc : "Calculate a mathematical expression",
    "cd" => builtin_cd : "Change the current directory\n    cd <path>",
    "contains" => contains : "Evaluates if the supplied argument contains a given string",
    "dirs" => builtin_dirs : "Display the current directory stack",
    "disown" => builtin_disown : DISOWN_DESC,
    "drop" => builtin_drop : "Delete a variable",
    "echo" => builtin_echo : "Display a line of text",
    "ends-with" => ends_with :"Evaluates if the supplied argument ends with a given string",
    "eval" => builtin_eval : "evaluates the evaluated expression",
    "exists" => builtin_exists : "Performs tests on files and text",
    "exit" => builtin_exit : "Exits the current session",
    "false" => builtin_false : "Do nothing, unsuccessfully",
    "fg" => builtin_fg : "Resumes and sets a background process as the active process",
    "fn" => builtin_fn : "Print list of functions",
    "help" => builtin_help : HELP_DESC,
    "history" => builtin_history : "Display a log of all commands previously executed",
    "ion-docs" => ion_docs : "Opens the Ion manual",
    "is" => builtin_is : "Simple alternative to == and !=",
    "jobs" => builtin_jobs : "Displays all jobs that are attached to the background",
    "matches" => builtin_matches : "Checks if a string matches a given regex",
    "not" => builtin_not : "Reverses the exit status value of the given command.",
    "or" => builtin_or : "Execute the command if the shell's previous status is failure",
    "popd" => builtin_popd : "Pop a directory from the stack",
    "pushd" => builtin_pushd : "Push a directory to the stack",
    "random" => builtin_random : "Outputs a random u64",
    "read" => builtin_read : "Read some variables\n    read <variable>",
    "set" => builtin_set : "Set or unset values of shell options and positional parameters.",
    "source" => builtin_source : SOURCE_DESC,
    "starts-with" => starts_with : "Evaluates if the supplied argument starts with a given string",
    "status" => builtin_status : "Evaluates the current runtime status",
    "suspend" => builtin_suspend : "Suspends the shell with a SIGTSTOP signal",
    "test" => builtin_test : "Performs tests on files and text",
    "true" => builtin_true : "Do nothing, successfully",
    "unalias" => builtin_unalias : "Delete an alias",
    "wait" => builtin_wait : "Waits until all running background processes have completed",
    "which" => builtin_which : "Shows the full path of commands"
);

/// Structure which represents a Terminal's command.
/// This command structure contains a name, and the code which run the
/// functionnality associated to this one, with zero, one or several argument(s).
pub struct Builtin {
    pub name: &'static str,
    pub help: &'static str,
    pub main: BuiltinFunction,
}

pub struct BuiltinMap {
    pub(crate) name:      &'static [&'static str],
    pub(crate) help:      &'static [&'static str],
    pub(crate) functions: &'static [BuiltinFunction],
}

impl BuiltinMap {
    pub fn get(&self, func: &str) -> Option<Builtin> {
        self.name.binary_search(&func).ok().map(|pos| unsafe {
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
fn builtin_status(args: &[&str], shell: &mut Shell) -> i32 {
    match status(args, shell) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.as_bytes());
            FAILURE
        }
    }
}

pub fn builtin_cd(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_CD) {
        return SUCCESS
    }

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

fn builtin_bool(args: &[&str], shell: &mut Shell) -> i32 {
    if args.len() != 2 {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        let _ = stderr.write_all(b"bool requires one argument\n");
        return FAILURE;
    }

    let opt = shell.variables.get_var(&args[1][1..]);
    let sh_var: &str = match opt.as_ref() {
        Some(s) => s,
        None => "",
    };

    match sh_var {
        "1" => (),
        "true" => (),
        _ => match args[1] {
            "1" => (),
            "true" => (),
            "--help" => print_man(MAN_BOOL),
            "-h" => print_man(MAN_BOOL),
            _ => return FAILURE,
        },
    }
    SUCCESS
}

fn builtin_is(args: &[&str], shell: &mut Shell) -> i32 {
    match is(args, shell) {
        Ok(()) => SUCCESS,
        Err(why) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = stderr.write_all(why.as_bytes());
            FAILURE
        }
    }
}

fn builtin_dirs(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_DIRS) {
        return SUCCESS
    }

    shell.directory_stack.dirs(args) 
}

fn builtin_pushd(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_PUSHD) {
        return SUCCESS
    }
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
    if check_help(args, MAN_POPD) {
        return SUCCESS
    }

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

fn builtin_fn(_: &[&str], shell: &mut Shell) -> i32 {
    fn_(&mut shell.functions)
}

fn builtin_read(args: &[&str], shell: &mut Shell) -> i32 { shell.variables.read(args) }

fn builtin_drop(args: &[&str], shell: &mut Shell) -> i32 {
    if args.len() >= 2 && args[1] == "-a" {
        drop_array(&mut shell.variables, args)
    } else {
        drop_variable(&mut shell.variables, args)
    }
}

fn builtin_set(args: &[&str], shell: &mut Shell) -> i32 { set::set(args, shell) }

fn builtin_eval(args: &[&str], shell: &mut Shell) -> i32 {
    let evaluated_command = args[1..].join(" ");
    let mut buffer = Terminator::new(evaluated_command);
    if buffer.is_terminated() {
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

fn builtin_random(args: &[&str], _: &mut Shell) -> i32 {
    match random::random(&args[1..]) {
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
    match job_control::disown(shell, &args[1..]) {
        Ok(()) => SUCCESS,
        Err(err) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: disown: {}", err);
            FAILURE
        }
    }
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

fn args_to_pipeline(args: &[&str]) -> Pipeline {
    let owned = args.into_iter().map(|&x| String::from(x)).collect::<Array>();
    let pipe_item = PipeItem::new(Job::new(owned, JobKind::And), Vec::new(), Vec::new());
    Pipeline { items: vec![pipe_item] }
}

fn builtin_not(args: &[&str], shell: &mut Shell) -> i32 {
    shell.run_pipeline(&mut args_to_pipeline(&args[1..]));
    match shell.previous_status {
        SUCCESS => FAILURE,
        FAILURE => SUCCESS,
        _ => shell.previous_status,
    }
}

fn builtin_and(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.previous_status {
        SUCCESS => {
            shell.run_pipeline(&mut args_to_pipeline(&args[1..]));
            shell.previous_status
        }
        _ => shell.previous_status,
    }
}

fn builtin_or(args: &[&str], shell: &mut Shell) -> i32 {
    match shell.previous_status {
        FAILURE => {
            shell.run_pipeline(&mut args_to_pipeline(&args[1..]));
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
        for path in env::var("PATH").unwrap_or("/bin".to_string()).split(sys::PATH_SEPARATOR) {
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
