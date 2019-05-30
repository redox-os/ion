pub mod man_pages;

mod command_info;
mod exec;
mod exists;
mod functions;
mod is;
mod job_control;
mod set;
mod source;
mod status;
mod variables;

use ion_builtins::{calc, conditionals, echo, random, test};

use self::{
    command_info::*,
    echo::echo,
    exec::exec,
    exists::exists,
    functions::print_functions,
    is::is,
    man_pages::*,
    source::source,
    status::status,
    test::test,
    variables::{alias, drop_alias, drop_array, drop_variable},
};

use std::{
    borrow::Cow,
    error::Error,
    io::{self, BufRead, Write},
    path::PathBuf,
};

use hashbrown::HashMap;
use liner::{Completer, Context};

use crate::{
    shell::{self, directory_stack::parse_numeric_arg, status::*, ProcessState, Shell},
    sys, types,
};
use itertools::Itertools;
use small;

const HELP_DESC: &str = "Display helpful information about a given command or list commands if \
                         none specified\n    help <command>";

const SOURCE_DESC: &str = "Evaluate the file following the command or re-initialize the init file";

const DISOWN_DESC: &str =
    "Disowning a process removes that process from the shell's background process table.";

/// The type for builtin functions. Builtins have direct access to the shell
pub type BuiltinFunction<'a> = &'a dyn Fn(&[small::String], &mut Shell) -> i32;

macro_rules! map {
    ($builtins:ident, $($name:expr => $func:ident: $help:expr),+) => {{
        $(
            $builtins.add($name, &$func, $help);
        )+
        $builtins
    }};
}

/// A container for builtins and their respective help text
///
/// Note: To reduce allocations, function are provided as pointer rather than boxed closures
/// ```
/// use ion_shell::builtins::BuiltinMap;
/// use ion_shell::Shell;
///
/// // create a builtin
/// let mut custom = |_args: &[small::String], _shell: &mut Shell| {
///     println!("Hello world!");
///     42
/// };
///
/// // create a builtin map with some predefined builtins
/// let mut builtins = BuiltinMap::new().with_basic().with_variables();
///
/// // add a builtin
/// builtins.add("custom builtin", &mut custom, "Very helpful comment to display to the user");
///
/// // execute a builtin
/// assert_eq!(
///     builtins.get("custom builtin").unwrap()(&["ion".into()], &mut Shell::new(false)),
///     42,
/// );
/// // >> Hello world!
pub struct BuiltinMap<'a> {
    fcts: HashMap<&'static str, BuiltinFunction<'a>>,
    help: HashMap<&'static str, &'static str>,
}

impl<'a> Default for BuiltinMap<'a> {
    fn default() -> Self {
        Self::with_capacity(64)
            .with_basic()
            .with_variables()
            .with_process_control()
            .with_values_tests()
            .with_files_and_directory()
    }
}

// Note for implementers:
// If you are implementing a builtin add it to the table below, create a well named manpage in
// man_pages and check for help flags by adding to the start of your builtin the following
// if check_help(args, MAN_BUILTIN_NAME) {
//     return SUCCESS
// }
impl<'a> BuiltinMap<'a> {
    /// Create a new, blank builtin map
    ///
    /// If you have a hint over the number of builtins, with_capacity is probably better
    pub fn new() -> Self { BuiltinMap { fcts: HashMap::new(), help: HashMap::new() } }

    /// Create a new, blank builtin map with a given capacity
    pub fn with_capacity(cap: usize) -> Self {
        BuiltinMap { fcts: HashMap::with_capacity(cap), help: HashMap::with_capacity(cap) }
    }

    /// Check if the given builtin exists
    pub fn contains(&self, func: &str) -> bool { self.fcts.get(&func).is_some() }

    /// Get the list of builtins included
    pub fn keys(&self) -> impl Iterator<Item = &str> { self.fcts.keys().cloned() }

    /// Get the provided help for a given builtin
    pub fn get_help(&self, func: &str) -> Option<&str> { self.help.get(func).cloned() }

    /// Get the function of a given builtin
    pub fn get(&self, func: &str) -> Option<BuiltinFunction<'a>> { self.fcts.get(func).cloned() }

    /// Add a new builtin
    pub fn add(&mut self, name: &'static str, func: BuiltinFunction<'a>, help: &'static str) {
        self.fcts.insert(name, func);
        self.help.insert(name, help);
    }

    /// Create and control variables
    ///
    /// Contains `fn`, `alias`, `unalias`, `drop`, `read`
    pub fn with_variables(mut self) -> Self {
        map!(
            self,
            "fn" => builtin_fn : "Print list of functions",
            "alias" => builtin_alias : "View, set or unset aliases",
            "unalias" => builtin_unalias : "Delete an alias",
            "drop" => builtin_drop : "Delete a variable",
            "read" => builtin_read : "Read some variables\n    read <variable>"
        )
    }

    /// Control subrpocesses states
    ///
    /// Contains `disown`, `bg`, `fg`, `wait`, `isatty`, `jobs`
    pub fn with_process_control(mut self) -> Self {
        map!(
            self,
            "disown" => builtin_disown : DISOWN_DESC,
            "bg" => builtin_bg : "Resumes a stopped background process",
            "fg" => builtin_fg : "Resumes and sets a background process as the active process",
            "wait" => builtin_wait : "Waits until all running background processes have completed",
            "isatty" => builtin_isatty : "Returns 0 exit status if the supplied FD is a tty",
            "jobs" => builtin_jobs : "Displays all jobs that are attached to the background"
        )
    }

    /// Utilities concerning the filesystem
    ///
    /// Contains `which`, `test`, `exists`, `popd`, `pushd`, `dirs`, `cd`
    pub fn with_files_and_directory(mut self) -> Self {
        map!(
            self,
            "which" => builtin_which : "Shows the full path of commands",
            "test" => builtin_test : "Performs tests on files and text",
            "exists" => builtin_exists : "Performs tests on files and text",
            "popd" => builtin_popd : "Pop a directory from the stack",
            "pushd" => builtin_pushd : "Push a directory to the stack",
            "dirs" => builtin_dirs : "Display the current directory stack",
            "cd" => builtin_cd : "Change the current directory\n    cd <path>"
        )
    }

    /// Utilities to test values
    ///
    /// Contains `bool`, `calc`, `eq`, `is`, `true`, `false`, `starts-with`, `ends-with`,
    /// `contains`, `matches`, `random`
    pub fn with_values_tests(mut self) -> Self {
        map!(
            self,
            "bool" => builtin_bool : "If the value is '1' or 'true', return 0 exit status",
            "calc" => builtin_calc : "Calculate a mathematical expression",
            "eq" => builtin_eq : "Simple alternative to == and !=",
            "is" => builtin_is : "Simple alternative to == and !=",
            "true" => builtin_true : "Do nothing, successfully",
            "false" => builtin_false : "Do nothing, unsuccessfully",
            "starts-with" => starts_with : "Evaluates if the supplied argument starts with a given string",
            "ends-with" => ends_with : "Evaluates if the supplied argument ends with a given string",
            "contains" => contains : "Evaluates if the supplied argument contains a given string",
            "matches" => builtin_matches : "Checks if a string matches a given regex",
            "random" => builtin_random : "Outputs a random u64"
        )
    }

    /// Basic utilities for any ion embedded library
    ///
    /// Contains `help`, `source`, `status`, `echo`, `type`
    pub fn with_basic(mut self) -> Self {
        map!(
            self,
            "help" => builtin_help : HELP_DESC,
            "source" => builtin_source : SOURCE_DESC,
            "status" => builtin_status : "Evaluates the current runtime status",
            "echo" => builtin_echo : "Display a line of text",
            "type" => builtin_type : "indicates how a command would be interpreted"
        )
    }

    /// Utilities specific for a shell, that should probably not be included in an embedded context
    ///
    /// Contains `eval`, `exec`, `exit`, `set`, `suspend`
    pub fn with_shell_dangerous(mut self) -> Self {
        map!(
            self,
            "eval" => builtin_eval : "Evaluates the evaluated expression",
            "exec" => builtin_exec : "Replace the shell with the given command.",
            "exit" => builtin_exit : "Exits the current session",
            "set" => builtin_set : "Set or unset values of shell options and positional parameters.",
            "suspend" => builtin_suspend : "Suspends the shell with a SIGTSTOP signal"
        )
    }
}

fn starts_with(args: &[small::String], _: &mut Shell) -> i32 { conditionals::starts_with(args) }
fn ends_with(args: &[small::String], _: &mut Shell) -> i32 { conditionals::ends_with(args) }
fn contains(args: &[small::String], _: &mut Shell) -> i32 { conditionals::contains(args) }

// Definitions of simple builtins go here
fn builtin_status(args: &[small::String], shell: &mut Shell) -> i32 {
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

pub fn builtin_cd(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_CD) {
        return SUCCESS;
    }

    match shell.cd(args.get(1)) {
        Ok(()) => {
            let _ = shell.fork_function("CD_CHANGE", &["ion"]);
            SUCCESS
        }
        Err(why) => {
            eprintln!("{}", why);
            FAILURE
        }
    }
}

fn builtin_bool(args: &[small::String], shell: &mut Shell) -> i32 {
    if args.len() != 2 {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        let _ = stderr.write_all(b"bool requires one argument\n");
        return FAILURE;
    }

    let opt =
        if args[1].is_empty() { None } else { shell.variables().get::<types::Str>(&args[1][1..]) };

    match opt.as_ref().map(types::Str::as_str) {
        Some("1") => (),
        Some("true") => (),
        _ => match &*args[1] {
            "1" => (),
            "true" => (),
            "--help" => println!("{}", MAN_BOOL),
            "-h" => println!("{}", MAN_BOOL),
            _ => return FAILURE,
        },
    }
    SUCCESS
}

fn builtin_is(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_IS) {
        return SUCCESS;
    }

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

fn builtin_dirs(args: &[small::String], shell: &mut Shell) -> i32 {
    // converts pbuf to an absolute path if possible
    fn try_abs_path(pbuf: &PathBuf) -> Cow<str> {
        Cow::Owned(
            pbuf.canonicalize().unwrap_or_else(|_| pbuf.clone()).to_string_lossy().to_string(),
        )
    }

    if check_help(args, MAN_DIRS) {
        return SUCCESS;
    }

    let mut clear = false; // -c
    let mut abs_pathnames = false; // -l
    let mut multiline = false; // -p | -v
    let mut index = false; // -v

    let mut num_arg = None;

    for arg in args.iter().skip(1) {
        match arg.as_ref() {
            "-c" => clear = true,
            "-l" => abs_pathnames = true,
            "-p" => multiline = true,
            "-v" => {
                index = true;
                multiline = true;
            }
            _ => num_arg = Some(arg),
        }
    }

    if clear {
        shell.clear_dir_stack();
    }

    let mapper: fn((usize, &PathBuf)) -> Cow<str> = match (abs_pathnames, index) {
        // ABS, INDEX
        (true, true) => |(num, x)| Cow::Owned(format!(" {}  {}", num, try_abs_path(x))),
        (true, false) => |(_, x)| try_abs_path(x),
        (false, true) => |(num, x)| Cow::Owned(format!(" {}  {}", num, x.to_string_lossy())),
        (false, false) => |(_, x)| x.to_string_lossy(),
    };

    let mut iter = shell.dir_stack().enumerate().map(mapper);

    if let Some(arg) = num_arg {
        let num = match parse_numeric_arg(arg.as_ref()) {
            Some((true, num)) => num,
            Some((false, num)) if shell.dir_stack().count() > num => {
                shell.dir_stack().count() - num - 1
            }
            _ => return FAILURE, /* Err(Cow::Owned(format!("ion: dirs: {}: invalid
                                  * argument\n", arg))) */
        };
        match iter.nth(num) {
            Some(x) => {
                println!("{}", x);
                SUCCESS
            }
            None => FAILURE,
        }
    } else {
        let folder: fn(String, Cow<str>) -> String =
            if multiline { |x, y| x + "\n" + &y } else { |x, y| x + " " + &y };

        if let Some(x) = iter.next() {
            println!("{}", iter.fold(x.to_string(), folder));
        }
        SUCCESS
    }
}

fn builtin_pushd(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_PUSHD) {
        return SUCCESS;
    }

    enum Action {
        Switch,          // <no arguments>
        RotLeft(usize),  // +[num]
        RotRight(usize), // -[num]
        Push(PathBuf),   // [dir]
    }

    let mut keep_front = false; // whether the -n option is present
    let mut action = Action::Switch;

    for arg in args {
        let arg = arg.as_ref();
        if arg == "-n" {
            keep_front = true;
        } else if let Action::Switch = action {
            // if action is not yet defined
            action = match parse_numeric_arg(arg) {
                Some((true, num)) => Action::RotLeft(num),
                Some((false, num)) => Action::RotRight(num),
                None => Action::Push(PathBuf::from(arg)), // no numeric arg => `dir`-parameter
            };
        } else {
            eprintln!("ion: pushd: too many arguments");
            return FAILURE;
        }
    }

    match action {
        Action::Switch => {
            if !keep_front {
                if let Err(why) = shell.swap(1) {
                    eprintln!("ion: pushd: {}", why);
                    return FAILURE;
                }
            }
        }
        Action::RotLeft(num) => {
            if !keep_front {
                if let Err(why) = shell.rotate_left(num) {
                    eprintln!("ion: pushd: {}", why);
                    return FAILURE;
                }
            }
        }
        Action::RotRight(num) => {
            if !keep_front {
                if let Err(why) = shell.rotate_right(num) {
                    eprintln!("ion: pushd: {}", why);
                    return FAILURE;
                }
            }
        }
        Action::Push(dir) => {
            if let Err(why) = shell.pushd(dir, keep_front) {
                eprintln!("ion: pushd: {}", why);
                return FAILURE;
            }
        }
    };

    println!(
        "{}",
        shell.dir_stack().map(|dir| dir.to_str().unwrap_or("ion: no directory found")).join(" ")
    );
    SUCCESS
}

fn builtin_popd(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_POPD) {
        return SUCCESS;
    }

    let len = shell.dir_stack().len();
    if len <= 1 {
        eprintln!("ion: popd: directory stack empty");
        return FAILURE;
    }

    let mut keep_front = false; // whether the -n option is present
    let mut index: usize = 0;

    for arg in args.iter().skip(1) {
        let arg = arg.as_ref();
        if arg == "-n" {
            keep_front = true;
        } else {
            let (count_from_front, num) = match parse_numeric_arg(arg) {
                Some(n) => n,
                None => {
                    eprintln!("ion: popd: {}: invalid argument", arg);
                    return FAILURE;
                }
            };

            index = if count_from_front {
                // <=> input number is positive
                num
            } else if let Some(n) = (len - 1).checked_sub(num) {
                n
            } else {
                eprintln!("ion: popd: negative directory stack index out of range");
                return FAILURE;
            };
        }
    }

    // apply -n
    if index == 0 && keep_front {
        index = 1;
    } else if index == 0 {
        // change to new directory, return if not possible
        if let Err(why) = shell.set_current_dir_by_index(1) {
            eprintln!("ion: popd: {}", why);
            return FAILURE;
        }
    }

    // pop element
    if shell.popd(index).is_some() {
        println!(
            "{}",
            shell
                .dir_stack()
                .map(|dir| dir.to_str().unwrap_or("ion: no directory found"))
                .join(" ")
        );
        SUCCESS
    } else {
        eprintln!("ion: popd: {}: directory stack index out of range", index);
        FAILURE
    }
}

fn builtin_alias(args: &[small::String], shell: &mut Shell) -> i32 {
    let args_str = args[1..].join(" ");
    alias(shell.variables_mut(), &args_str)
}

fn builtin_unalias(args: &[small::String], shell: &mut Shell) -> i32 {
    drop_alias(shell.variables_mut(), args)
}

// TODO There is a man page for fn however the -h and --help flags are not
// checked for.
fn builtin_fn(_: &[small::String], shell: &mut Shell) -> i32 { print_functions(shell.variables()) }

struct EmptyCompleter;

impl Completer for EmptyCompleter {
    fn completions(&mut self, _start: &str) -> Vec<String> { Vec::new() }
}

fn builtin_read(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_READ) {
        return SUCCESS;
    }

    if sys::isatty(sys::STDIN_FILENO) {
        let mut con = Context::new();
        for arg in args.iter().skip(1) {
            match con.read_line(format!("{}=", arg.trim()), None, &mut EmptyCompleter) {
                Ok(buffer) => {
                    shell.variables_mut().set(arg.as_ref(), buffer.trim());
                }
                Err(_) => return FAILURE,
            }
        }
    } else {
        let stdin = io::stdin();
        let handle = stdin.lock();
        let mut lines = handle.lines();
        for arg in args.iter().skip(1) {
            if let Some(Ok(line)) = lines.next() {
                shell.variables_mut().set(arg.as_ref(), line.trim());
            }
        }
    }
    SUCCESS
}

fn builtin_drop(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_DROP) {
        return SUCCESS;
    }
    if args.len() >= 2 && args[1] == "-a" {
        drop_array(shell.variables_mut(), args)
    } else {
        drop_variable(shell.variables_mut(), args)
    }
}

fn builtin_set(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_SET) {
        return SUCCESS;
    }
    set::set(args, shell)
}

fn builtin_eq(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_EQ) {
        return SUCCESS;
    }

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

fn builtin_eval(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_EVAL) {
        SUCCESS
    } else {
        shell.execute_command(args[1..].join(" ").as_bytes()).unwrap_or_else(|_| {
            eprintln!("ion: supplied eval expression was not terminated");
            FAILURE
        })
    }
}

fn builtin_source(args: &[small::String], shell: &mut Shell) -> i32 {
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

fn builtin_echo(args: &[small::String], _: &mut Shell) -> i32 {
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

fn builtin_test(args: &[small::String], _: &mut Shell) -> i32 {
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
fn builtin_calc(args: &[small::String], _: &mut Shell) -> i32 {
    match calc::calc(&args[1..]) {
        Ok(()) => SUCCESS,
        Err(why) => {
            eprintln!("{}", why);
            FAILURE
        }
    }
}

fn builtin_random(args: &[small::String], _: &mut Shell) -> i32 {
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

fn builtin_true(args: &[small::String], _: &mut Shell) -> i32 {
    check_help(args, MAN_TRUE);
    SUCCESS
}

fn builtin_false(args: &[small::String], _: &mut Shell) -> i32 {
    if check_help(args, MAN_FALSE) {
        return SUCCESS;
    }
    FAILURE
}

// TODO create a manpage
fn builtin_wait(_: &[small::String], shell: &mut Shell) -> i32 {
    shell.wait_for_background();
    SUCCESS
}

fn builtin_jobs(args: &[small::String], shell: &mut Shell) -> i32 {
    check_help(args, MAN_JOBS);
    job_control::jobs(shell);
    SUCCESS
}

fn builtin_bg(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_BG) {
        return SUCCESS;
    }
    job_control::bg(shell, &args[1..])
}

fn builtin_fg(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_FG) {
        return SUCCESS;
    }
    job_control::fg(shell, &args[1..])
}

fn builtin_suspend(args: &[small::String], _: &mut Shell) -> i32 {
    if check_help(args, MAN_SUSPEND) {
        return SUCCESS;
    }
    shell::signals::suspend(0);
    SUCCESS
}

fn builtin_disown(args: &[small::String], shell: &mut Shell) -> i32 {
    for arg in args {
        if *arg == "--help" {
            println!("{}", MAN_DISOWN);
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

fn builtin_help(args: &[small::String], shell: &mut Shell) -> i32 {
    let builtins = shell.builtins();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if let Some(command) = args.get(1) {
        if let Some(help) = builtins.get_help(command) {
            let _ = stdout.write_all(help.as_bytes());
            let _ = stdout.write_all(b"\n");
        } else {
            let _ = stdout.write_all(b"Command helper not found [run 'help']...");
            let _ = stdout.write_all(b"\n");
        }
    } else {
        let commands = builtins.keys();

        let mut buffer: Vec<u8> = Vec::new();
        for command in commands {
            let _ = writeln!(buffer, "{}", command);
        }
        let _ = stdout.write_all(&buffer);
    }
    SUCCESS
}

fn builtin_exit(args: &[small::String], shell: &mut Shell) -> i32 {
    if check_help(args, MAN_EXIT) {
        return SUCCESS;
    }
    // Kill all active background tasks before exiting the shell.
    for process in shell.background.lock().unwrap().iter() {
        if process.state != ProcessState::Empty {
            let _ = sys::kill(process.pid, sys::SIGTERM);
        }
    }
    shell.exit(args.get(1).and_then(|status| status.parse::<i32>().ok()))
}

fn builtin_exec(args: &[small::String], shell: &mut Shell) -> i32 {
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
fn builtin_matches(args: &[small::String], _: &mut Shell) -> i32 {
    if check_help(args, MAN_MATCHES) {
        return SUCCESS;
    }
    if args[1..].len() != 2 {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        let _ = stderr.write_all(b"match takes two arguments\n");
        return BAD_ARG;
    }
    let input = &args[1];
    let re = match Regex::new(&args[2]) {
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

fn builtin_exists(args: &[small::String], shell: &mut Shell) -> i32 {
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

fn builtin_which(args: &[small::String], shell: &mut Shell) -> i32 {
    match which(args, shell) {
        Ok(result) => result,
        Err(()) => FAILURE,
    }
}

fn builtin_type(args: &[small::String], shell: &mut Shell) -> i32 {
    match find_type(args, shell) {
        Ok(result) => result,
        Err(()) => FAILURE,
    }
}

fn builtin_isatty(args: &[small::String], _: &mut Shell) -> i32 {
    if check_help(args, MAN_ISATTY) {
        return SUCCESS;
    }

    if args.len() > 1 {
        // sys::isatty expects a usize if compiled for redox but otherwise a i32.
        #[cfg(target_os = "redox")]
        match args[1].parse::<usize>() {
            Ok(r) => {
                if sys::isatty(r) {
                    return SUCCESS;
                }
            }
            Err(_) => eprintln!("ion: isatty given bad number"),
        }

        #[cfg(not(target_os = "redox"))]
        match args[1].parse::<i32>() {
            Ok(r) => {
                if sys::isatty(r) {
                    return SUCCESS;
                }
            }
            Err(_) => eprintln!("ion: isatty given bad number"),
        }
    } else {
        return SUCCESS;
    }

    FAILURE
}
