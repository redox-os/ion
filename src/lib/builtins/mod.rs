pub mod source;
pub mod variables;
pub mod functions;
pub mod calc;
pub mod random;

mod command_info;
mod conditionals;
mod job_control;
mod man_pages;
mod test;
mod echo;
mod set;
mod status;
mod exists;
mod exec;
mod ion;
mod is;

use self::command_info::*;
use self::conditionals::{contains, ends_with, starts_with};
use self::echo::echo;
use self::exec::exec;
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

use parser::Terminator;
use parser::pipelines::{PipeItem, Pipeline};
use shell::job_control::{JobControl, ProcessState};
use shell::fork_function::fork_function;
use shell::status::*;
use shell::{self, FlowLogic, Job, JobKind, Shell, ShellHistory};
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

/// If you are implementing a builtin add it to the table below, create a well named manpage in
/// man_pages and check for help flags by adding to the start of your builtin the following
/// if check_help(args, MAN_BUILTIN_NAME) {
///     return SUCCESS
/// }

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
    "eval" => builtin_eval : "Evaluates the evaluated expression",
    "exec" => builtin_exec : "Replace the shell with the given command.",
    "exists" => builtin_exists : "Performs tests on files and text",
    "exit" => builtin_exit : "Exits the current session",
    "false" => builtin_false : "Do nothing, unsuccessfully",
    "fg" => builtin_fg : "Resumes and sets a background process as the active process",
    "fn" => builtin_fn : "Print list of functions",
    "help" => builtin_help : HELP_DESC,
    "history" => builtin_history : "Display a log of all commands previously executed",
    "ion-docs" => ion_docs : "Opens the Ion manual",
    "is" => builtin_is : "Simple alternative to == and !=",
    "isatty" => builtin_isatty : "Returns 0 exit status if the supplied FD is a tty",
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
    "type" => builtin_type : "indicates how a command would be interpreted",
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
        return SUCCESS;
    }

    match shell.directory_stack.cd(args, &shell.variables) {
        Ok(()) => {
            match env::current_dir() {
                Ok(cwd) => {
                    let pwd = shell.get_var_or_empty("PWD");
                    let pwd = &pwd;
                    let current_dir = cwd.to_str().unwrap_or("?");

                    if pwd != current_dir {
                        shell.set_var("OLDPWD", pwd);
                        shell.set_var("PWD", current_dir);
                    }
                    fork_function(shell, "CD_CHANGE", &["ion"]);
                },
                Err(_) => env::set_var("PWD", "?"),
            };
            SUCCESS
        }
        Err(why) => {
            eprintln!("{}", why);
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
        return SUCCESS;
    }

    shell.directory_stack.dirs(args)
}

fn builtin_pushd(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_PUSHD) {
        return SUCCESS;
    }
    match shell.directory_stack.pushd(args, &mut shell.variables) {
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
        return SUCCESS;
    }
    match shell.directory_stack.popd(args, &mut shell.variables) {
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

// TODO There is a man page for fn however the -h and --help flags are not
// checked for.
fn builtin_fn(_: &[&str], shell: &mut Shell) -> i32 { fn_(&mut shell.functions) }

fn builtin_read(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_READ) {
        return SUCCESS;
    }
    shell.variables.read(args)
}

fn builtin_drop(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_DROP) {
        return SUCCESS;
    }
    if args.len() >= 2 && args[1] == "-a" {
        drop_array(&mut shell.variables, args)
    } else {
        drop_variable(&mut shell.variables, args)
    }
}

fn builtin_set(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_SET) {
        return SUCCESS;
    }
    set::set(args, shell)
}

fn builtin_eval(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_EVAL) {
        return SUCCESS;
    }
    let evaluated_command = args[1..].join(" ");
    let mut buffer = Terminator::new(evaluated_command);
    if buffer.is_terminated() {
        shell.on_command(&buffer.consume());
        shell.previous_status
    } else {
        eprintln!("ion: supplied eval expression was not terminted");
        FAILURE
    }
}

fn builtin_history(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_HISTORY) {
        return SUCCESS;
    }
    shell.print_history(args)
}

fn builtin_source(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_SOURCE) {
        return SUCCESS;
    }
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
    if check_help(args, MAN_ECHO) {
        return SUCCESS;
    }
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
    // Do not use `check_help` for the `test` builtin. The
    // `test` builtin contains a "-h" option.
    match test(args) {
        Ok(true) => SUCCESS,
        Ok(false) => FAILURE,
        Err(why) => {
            eprintln!("{}", why);
            FAILURE
        }
    }
}

// TODO create manpage.
fn builtin_calc(args: &[&str], _: &mut Shell) -> i32 {
    match calc::calc(&args[1..]) {
        Ok(()) => SUCCESS,
        Err(why) => {
            eprintln!("{}", why);
            FAILURE
        }
    }
}

fn builtin_random(args: &[&str], _: &mut Shell) -> i32 {
    if check_help(args, MAN_RANDOM) {
        return SUCCESS;
    }
    match random::random(&args[1..]) {
        Ok(()) => SUCCESS,
        Err(why) => {
            eprintln!("{}", why);
            FAILURE
        }
    }
}

fn builtin_true(args: &[&str], _: &mut Shell) -> i32 {
    check_help(args, MAN_TRUE);
    SUCCESS
}

fn builtin_false(args: &[&str], _: &mut Shell) -> i32 {
    if check_help(args, MAN_FALSE) {
        return SUCCESS;
    }
    FAILURE
}

// TODO create a manpage
fn builtin_wait(_: &[&str], shell: &mut Shell) -> i32 {
    shell.wait_for_background();
    SUCCESS
}

fn builtin_jobs(args: &[&str], shell: &mut Shell) -> i32 {
    check_help(args, MAN_JOBS);
    job_control::jobs(shell);
    SUCCESS
}

fn builtin_bg(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_BG) {
        return SUCCESS;
    }
    job_control::bg(shell, &args[1..])
}

fn builtin_fg(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_FG) {
        return SUCCESS;
    }
    job_control::fg(shell, &args[1..])
}

fn builtin_suspend(args: &[&str], _: &mut Shell) -> i32 {
    if check_help(args, MAN_SUSPEND) {
        return SUCCESS;
    }
    shell::signals::suspend(0);
    SUCCESS
}

fn builtin_disown(args: &[&str], shell: &mut Shell) -> i32 {
    for arg in args {
        if *arg == "--help" {
            print_man(MAN_DISOWN);
            return SUCCESS;
        }
    }
    match job_control::disown(shell, &args[1..]) {
        Ok(()) => SUCCESS,
        Err(err) => {
            eprintln!("ion: disown: {}", err);
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
    if check_help(args, MAN_EXIT) {
        return SUCCESS;
    }
    // Kill all active background tasks before exiting the shell.
    for process in shell.background.lock().unwrap().iter() {
        if process.state != ProcessState::Empty {
            let _ = sys::kill(process.pid, sys::SIGTERM);
        }
    }
    let previous_status = shell.previous_status;
    shell.exit(
        args.get(1)
            .and_then(|status| status.parse::<i32>().ok())
            .unwrap_or(previous_status),
    )
}

fn builtin_exec(args: &[&str], shell: &mut Shell) -> i32 {
    match exec(shell, &args[1..]) {
        // Shouldn't ever hit this case.
        Ok(()) => SUCCESS,
        Err(err) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            let _ = writeln!(stderr, "ion: exec: {}", err);
            FAILURE
        }
    }
}

use regex::Regex;
fn builtin_matches(args: &[&str], _: &mut Shell) -> i32 {
    if check_help(args, MAN_MATCHES) {
        return SUCCESS;
    }
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
    let owned = args.into_iter()
        .map(|&x| String::from(x))
        .collect::<Array>();
    let pipe_item = PipeItem::new(Job::new(owned, JobKind::And), Vec::new(), Vec::new());
    Pipeline {
        items: vec![pipe_item],
    }
}

fn builtin_not(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_NOT) {
        return SUCCESS;
    }
    if args.len() < 2 {
        eprintln!("not requires at least one argument");
        return FAILURE;
    }
    shell.run_pipeline(&mut args_to_pipeline(&args[1..]));
    match shell.previous_status {
        SUCCESS => FAILURE,
        FAILURE => SUCCESS,
        _ => shell.previous_status,
    }
}

fn builtin_and(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_AND) {
        return SUCCESS;
    }
    if args.len() < 2 {
        eprintln!("and requires at least one argument");
        return FAILURE;
    }
    match shell.previous_status {
        SUCCESS => {
            shell.run_pipeline(&mut args_to_pipeline(&args[1..]));
            shell.previous_status
        }
        _ => shell.previous_status,
    }
}

fn builtin_or(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_OR) {
        return SUCCESS;
    }
    if args.len() < 2 {
        eprintln!("or requires at least one argument");
        return FAILURE;
    }
    match shell.previous_status {
        FAILURE => {
            shell.run_pipeline(&mut args_to_pipeline(&args[1..]));
            shell.previous_status
        }
        _ => shell.previous_status,
    }
}

fn builtin_exists(args: &[&str], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_EXISTS) {
        return SUCCESS;
    }
    match exists(args, shell) {
        Ok(true) => SUCCESS,
        Ok(false) => FAILURE,
        Err(why) => {
            eprintln!("{}", why);
            FAILURE
        }
    }
}

fn builtin_which(args: &[&str], shell: &mut Shell) -> i32 {
    match which(args, shell) {
        Ok(result) => result,
        Err(()) => FAILURE
    }
}

fn builtin_type(args: &[&str], shell: &mut Shell) -> i32 {
    match find_type(args, shell) {
        Ok(result) => result,
        Err(()) => FAILURE
    }
}

fn builtin_isatty(args: &[&str], _: &mut Shell) -> i32 {
    if check_help(args, MAN_ISATTY) {
        return SUCCESS
    }

    if args.len() > 1 {
        // sys::isatty expects a usize if compiled for redox but otherwise a i32.
        #[cfg(target_os = "redox")]
        match args[1].parse::<usize>() {
            Ok(r) => if sys::isatty(r) {
                return SUCCESS
            },
            Err(_) => eprintln!("ion: isatty given bad number")
        }

        #[cfg(not(target_os = "redox"))]
        match args[1].parse::<i32>() {
            Ok(r) => if sys::isatty(r) {
                return SUCCESS
            },
            Err(_) => eprintln!("ion: isatty given bad number")
        }
    } else {
        return SUCCESS
    }

    FAILURE
}
