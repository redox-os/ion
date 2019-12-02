/// helpers for creating help
pub mod man_pages;

mod command_info;
mod conditionals;
mod echo;
mod exists;
mod functions;
mod helpers;
mod is;
mod job_control;
mod math;
mod random;
mod set;
mod source;
mod status;
mod test;
mod variables;

pub use self::{
    command_info::builtin_which,
    conditionals::{builtin_contains, builtin_ends_with, builtin_starts_with},
    echo::builtin_echo,
    exists::builtin_exists,
    functions::builtin_fn_,
    helpers::Status,
    is::builtin_is,
    man_pages::check_help,
    math::builtin_math,
    set::builtin_set,
    source::builtin_source,
    status::builtin_status,
    test::builtin_test,
    variables::{builtin_alias, builtin_drop, builtin_unalias},
};
use crate as ion_shell;
use crate::{
    shell::{Shell, Value},
    types,
};
use builtins_proc::builtin;
use itertools::Itertools;
use liner::{Completer, Context, Prompt};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

const HELP_DESC: &str = "Display helpful information about a given command or list commands if \
                         none specified\n    help <command>";

const SOURCE_DESC: &str = "Evaluate the file following the command or re-initialize the init file";

const DISOWN_DESC: &str =
    "Disowning a process removes that process from the shell's background process table.";

/// The type for builtin functions. Builtins have direct access to the shell
pub type BuiltinFunction<'a> = &'a dyn Fn(&[types::Str], &mut Shell<'_>) -> Status;

// parses -N or +N patterns
// required for popd, pushd, dirs
fn parse_numeric_arg(arg: &str) -> Option<(bool, usize)> {
    let b = match arg.chars().nth(0) {
        Some('+') => Some(true),
        Some('-') => Some(false),
        _ => None,
    }?;
    let num = arg[1..].parse::<usize>().ok()?;
    Some((b, num))
}

/// A container for builtins and their respective help text
///
/// Note: To reduce allocations, function are provided as pointer rather than boxed closures
/// ```
/// use ion_shell::{types, Shell, builtins::{BuiltinMap, Status}};
///
/// // create a builtin
/// let mut custom = |_args: &[types::Str], _shell: &mut Shell| {
///     println!("Hello world!");
///     Status::error("Can't proceed")
/// };
///
/// // create a builtin map with some predefined builtins
/// let mut builtins = BuiltinMap::new();
/// builtins.with_basic().with_variables();
///
/// // add a builtin
/// builtins.add("custom builtin", &mut custom, "Very helpful comment to display to the user");
///
/// // execute a builtin
/// assert!(
///     builtins.get("custom builtin").unwrap()(&["ion".into()], &mut Shell::new()).is_failure(),
/// );
/// // >> Hello world!
pub struct BuiltinMap<'a> {
    fcts: HashMap<&'static str, BuiltinFunction<'a>>,
    help: HashMap<&'static str, &'static str>,
}

impl<'a> Default for BuiltinMap<'a> {
    fn default() -> Self {
        let mut builtins = Self::with_capacity(64);
        builtins
            .with_basic()
            .with_variables()
            .with_process_control()
            .with_values_tests()
            .with_files_and_directory();
        builtins
    }
}

// Note for implementers:
// If you are implementing a builtin add it to the table below, create a well named manpage in
// man_pages and check for help flags by adding to the start of your builtin the following
// if check_help(args, MAN_BUILTIN_NAME) {
//     return Status::SUCCESS
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
    pub fn keys(&self) -> impl Iterator<Item = &str> { self.fcts.keys().copied() }

    /// Get the provided help for a given builtin
    pub fn get_help(&self, func: &str) -> Option<&str> { self.help.get(func).copied() }

    /// Get the function of a given builtin
    pub fn get(&self, func: &str) -> Option<BuiltinFunction<'a>> { self.fcts.get(func).cloned() }

    /// Add a new builtin
    pub fn add(
        &mut self,
        name: &'static str,
        func: BuiltinFunction<'a>,
        help: &'static str,
    ) -> &mut Self {
        self.fcts.insert(name, func);
        self.help.insert(name, help);
        self
    }

    /// Create and control variables
    ///
    /// Contains `fn`, `alias`, `unalias`, `drop`, `read`
    pub fn with_variables(&mut self) -> &mut Self {
        self.add("fn", &builtin_fn_, "Print list of functions")
            .add("alias", &builtin_alias, "View, set or unset aliases")
            .add("unalias", &builtin_unalias, "Delete an alias")
            .add("drop", &builtin_drop, "Delete a variable")
            .add("read", &builtin_read, "Read some variables\n    read <variable>")
    }

    /// Control subrpocesses states
    ///
    /// Contains `disown`, `bg`, `fg`, `wait`, `isatty`, `jobs`
    pub fn with_process_control(&mut self) -> &mut Self {
        self.add("disown", &builtin_disown, DISOWN_DESC)
            .add("bg", &builtin_bg, "Resumes a stopped background process")
            .add("fg", &builtin_fg, "Resumes and sets a background process as the active process")
            .add(
                "wait",
                &builtin_wait,
                "Waits until all running background processes have completed",
            )
            .add("isatty", &builtin_isatty, "Returns 0 exit status if the supplied FD is a tty")
            .add("jobs", &builtin_jobs, "Displays all jobs that are attached to the background")
    }

    /// Utilities concerning the filesystem
    ///
    /// Contains `test`, `exists`, `popd`, `pushd`, `dirs`, `cd`
    pub fn with_files_and_directory(&mut self) -> &mut Self {
        self.add("test", &builtin_test, "Performs tests on files and text")
            .add("exists", &builtin_exists, "Performs tests on files and text")
            .add("popd", &builtin_popd, "Pop a directory from the stack")
            .add("pushd", &builtin_pushd, "Push a directory to the stack")
            .add("dirs", &builtin_dirs, "Display the current directory stack")
            .add("cd", &builtin_cd, "Change the current directory\n    cd <path>")
            .add("dir_depth", &builtin_dir_depth, "Set the maximum directory depth")
    }

    /// Utilities to test values
    ///
    /// Contains `bool`, `math`, `eq`, `is`, `true`, `false`, `starts-with`, `ends-with`,
    /// `contains`, `matches`, `random`
    pub fn with_values_tests(&mut self) -> &mut Self {
        self.add("bool", &builtin_bool, "If the value is '1' or 'true', return 0 exit status")
            .add("math", &builtin_math, "Calculate a mathematical expression")
            .add("eq", &builtin_is, "Simple alternative to == and !=")
            .add("is", &builtin_is, "Simple alternative to == and !=")
            .add("true", &builtin_true_, "Do nothing, successfully")
            .add("false", &builtin_false_, "Do nothing, unsuccessfully")
            .add(
                "starts-with",
                &builtin_starts_with,
                "Evaluates if the supplied argument starts with a given string",
            )
            .add(
                "ends-with",
                &builtin_ends_with,
                "Evaluates if the supplied argument ends with a given string",
            )
            .add(
                "contains",
                &builtin_contains,
                "Evaluates if the supplied argument contains a given string",
            )
            .add("matches", &builtin_matches, "Checks if a string matches a given regex")
            .add("random", &builtin_random, "Outputs a random u64")
    }

    /// Basic utilities for any ion embedded library
    ///
    /// Contains `help`, `source`, `status`, `echo`, `type`, `which`
    pub fn with_basic(&mut self) -> &mut Self {
        self.add("help", &builtin_help, HELP_DESC)
            .add("source", &builtin_source, SOURCE_DESC)
            .add("status", &builtin_status, "Evaluates the current runtime status")
            .add("echo", &builtin_echo, "Display a line of text")
            .add("which", &builtin_which, "indicates what would be called for a given command")
            .add("type", &builtin_which, "indicates what would be called for a given command")
    }

    /// Utilities that may be a security risk. Not included by default
    ///
    /// Contains `eval`, `set`
    pub fn with_unsafe(&mut self) -> &mut Self {
        self.add("eval", &builtin_eval, "Evaluates the evaluated expression").add(
            "set",
            &builtin_set,
            "Set or unset values of shell options and positional parameters.",
        )
    }
}

#[builtin(
    desc = "set the dir stack depth",
    man = "
SYNOPSYS
    dir_depth [DEPTH]

DESCRIPTION
    If DEPTH is given, set the dir stack max depth to DEPTH, else remove the limit"
)]
pub fn dir_depth(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let depth = match args.get(1) {
        None => None,
        Some(arg) => match arg.parse::<usize>() {
            Ok(num) => Some(num),
            Err(_) => return Status::error("dir_depth's argument must be a positive integer"),
        },
    };
    shell.dir_stack_mut().set_max_depth(depth);
    Status::SUCCESS
}

#[builtin(
    desc = "Change directory.",
    man = "
SYNOPSIS
    cd DIRECTORY

DESCRIPTION
    Without arguments cd changes the working directory to your home directory.
    With arguments cd changes the working directory to the directory you provided.
"
)]
pub fn cd(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let err = match args.get(1) {
        Some(dir) => {
            let dir = dir.as_str();
            if let Some(Value::Array(cdpath)) = shell.variables().get("CDPATH").cloned() {
                if dir == "-" {
                    shell.dir_stack_mut().switch_to_previous_directory()
                } else {
                    let check_cdpath_first = cdpath
                        .iter()
                        .map(|path| {
                            let path_dir = Path::new(&path.to_string()).join(dir);
                            shell.dir_stack_mut().change_and_push_dir(&path_dir)
                        })
                        .find(Result::is_ok)
                        .unwrap_or_else(|| {
                            shell.dir_stack_mut().change_and_push_dir(Path::new(dir))
                        });
                    shell.dir_stack_mut().popd(1);
                    check_cdpath_first
                }
            } else {
                shell.dir_stack_mut().change_and_push_dir(Path::new(dir))
            }
        }
        None => shell.dir_stack_mut().switch_to_home_directory(),
    };

    match err {
        Ok(()) => {
            if let Some(Value::Function(function)) = shell.variables().get("CD_CHANGE").cloned() {
                let _ = shell.execute_function(&function, &["ion"]);
            }
            Status::SUCCESS
        }
        Err(why) => Status::error(format!("{}", why)),
    }
}

#[builtin(
    desc = "Returns true if the value given to it is equal to '1' or 'true'.",
    man = "
SYNOPSIS
    bool VALUE

DESCRIPTION
    Returns true if the value given to it is equal to '1' or 'true'.
"
)]
pub fn bool(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if args.len() != 2 {
        return Status::error("bool requires one argument");
    }

    let opt = if args[1].is_empty() { None } else { shell.variables().get_str(&args[1][1..]).ok() };

    match opt.as_ref().map(types::Str::as_str) {
        Some("1") | Some("true") => Status::TRUE,
        _ if ["1", "true"].contains(&args[1].as_ref()) => Status::TRUE,
        _ => Status::FALSE,
    }
}

#[builtin(
    desc = "prints the directory stack",
    man = "
SYNOPSIS
    dirs

DESCRIPTION
    dirs prints the current directory stack.
"
)]
pub fn dirs(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    // converts pbuf to an absolute path if possible
    fn try_abs_path(pbuf: &PathBuf) -> Cow<'_, str> {
        Cow::Owned(
            pbuf.canonicalize().unwrap_or_else(|_| pbuf.clone()).to_string_lossy().to_string(),
        )
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
        shell.dir_stack_mut().clear();
    }

    let mapper: fn((usize, &PathBuf)) -> Cow<'_, str> = match (abs_pathnames, index) {
        // ABS, INDEX
        (true, true) => |(num, x)| Cow::Owned(format!(" {}  {}", num, try_abs_path(x))),
        (true, false) => |(_, x)| try_abs_path(x),
        (false, true) => |(num, x)| Cow::Owned(format!(" {}  {}", num, x.to_string_lossy())),
        (false, false) => |(_, x)| x.to_string_lossy(),
    };

    let mut iter = shell.dir_stack().dirs();

    if let Some(arg) = num_arg {
        let num = match parse_numeric_arg(arg.as_ref()) {
            Some((true, num)) => num,
            Some((false, num)) if shell.dir_stack().dirs().count() > num => {
                shell.dir_stack().dirs().count() - num - 1
            }
            _ => return Status::error(format!("ion: dirs: {}: invalid argument", arg)),
        };
        match iter.nth(num).map(|x| mapper((num, x))) {
            Some(x) => {
                println!("{}", x);
                Status::SUCCESS
            }
            None => Status::error(""),
        }
    } else {
        println!("{}", iter.enumerate().map(mapper).format(if multiline { "\n" } else { " " }));
        Status::SUCCESS
    }
}

#[builtin(
    desc = "push a directory to the directory stack",
    man = "
SYNOPSIS
    pushd DIRECTORY

DESCRIPTION
    pushd pushes a directory to the directory stack.
"
)]
pub fn pushd(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    enum Action {
        Switch,          // <no arguments>
        RotLeft(usize),  // +[num]
        RotRight(usize), // -[num]
        Push(PathBuf),   // [dir]
    }

    let mut keep_front = false; // whether the -n option is present
    let mut action = Action::Switch;

    for arg in args.iter().skip(1) {
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
            return Status::error("ion: pushd: too many arguments");
        }
    }

    match action {
        Action::Switch => {
            if !keep_front {
                if let Err(why) = shell.dir_stack_mut().swap(1) {
                    return Status::error(format!("ion: pushd: {}", why));
                }
            }
        }
        Action::RotLeft(num) => {
            if !keep_front {
                if let Err(why) = shell.dir_stack_mut().rotate_left(num) {
                    return Status::error(format!("ion: pushd: {}", why));
                }
            }
        }
        Action::RotRight(num) => {
            if !keep_front {
                if let Err(why) = shell.dir_stack_mut().rotate_right(num) {
                    return Status::error(format!("ion: pushd: {}", why));
                }
            }
        }
        Action::Push(dir) => {
            if let Err(why) = shell.dir_stack_mut().pushd(&dir, keep_front) {
                return Status::error(format!("ion: pushd: {}", why));
            }
        }
    };

    println!(
        "{}",
        shell
            .dir_stack()
            .dirs()
            .map(|dir| dir.to_str().unwrap_or("ion: no directory found"))
            .format(" ")
    );
    Status::SUCCESS
}

#[builtin(
    desc = "shift through the directory stack",
    man = "
SYNOPSIS
    popd

DESCRIPTION
    popd removes the top directory from the directory stack and changes the working directory to \
           the new top directory.
    pushd adds directories to the stack.
"
)]
pub fn popd(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let len = shell.dir_stack().dirs().len();
    if len <= 1 {
        return Status::error("ion: popd: directory stack empty");
    }

    let mut keep_front = false; // whether the -n option is present
    let mut index: usize = 0;

    for arg in args.iter().skip(1) {
        let arg = arg.as_ref();
        if arg == "-n" {
            keep_front = true;
        } else if let Some((count_from_front, num)) = parse_numeric_arg(arg) {
            index = if count_from_front {
                // <=> input number is positive
                num
            } else if let Some(n) = (len - 1).checked_sub(num) {
                n
            } else {
                return Status::error("ion: popd: negative directory stack index out of range");
            };
        }

        // apply -n
        if index == 0 && keep_front {
            index = 1;
        } else if index == 0 {
            // change to new directory, return if not possible
            if let Err(why) = shell.dir_stack_mut().set_current_dir_by_index(1) {
                return Status::error(format!("ion: popd: {}", why));
            } else {
                return Status::error(format!("ion: popd: {}: invalid argument", arg));
            };
        }
    }

    // pop element
    let dir_stack = shell.dir_stack_mut();
    if dir_stack.popd(index).is_some() {
        if let Err(err) = dir_stack.set_current_dir_by_index(0) {
            return Status::error(format!("ion: popd: {}", err));
        }
        println!("{}", shell.dir_stack().dirs().map(|dir| dir.display()).format(" "));
        Status::SUCCESS
    } else {
        Status::error(format!("ion: popd: {}: directory stack index out of range", index))
    }
}

struct EmptyCompleter;

impl Completer for EmptyCompleter {
    fn completions(&mut self, _start: &str) -> Vec<String> { Vec::new() }
}

#[builtin(
    desc = "read a line of input into some variables",
    man = "
SYNOPSIS
    read VARIABLES...

DESCRIPTION
    For each variable reads from standard input and stores the results in the variable.
"
)]
pub fn read(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if atty::is(atty::Stream::Stdin) {
        let mut con = Context::new();
        for arg in args.iter().skip(1) {
            match con.read_line(Prompt::from(format!("{}=", arg.trim())), None, &mut EmptyCompleter)
            {
                Ok(buffer) => {
                    shell.variables_mut().set(arg.as_ref(), buffer.trim());
                }
                Err(_) => return Status::FALSE,
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
    Status::SUCCESS
}

#[builtin(
    desc = "evaluates the specified commands",
    man = "
SYNOPSIS
    eval COMMANDS...

DESCRIPTION
    eval evaluates the given arguments as a command. If more than one argument is given,
    all arguments are joined using a space as a separator."
)]
pub fn eval(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    shell.execute_command(args[1..].join(" ").as_bytes()).unwrap_or_else(|_| {
        Status::error("ion: supplied eval expression was not terminated".to_string())
    })
}

#[builtin(
    desc = "generate a random number",
    man = "
SYNOPSIS
    random
    random START END

DESCRIPTION
    random generates a pseudo-random integer. IT IS NOT SECURE.
    The range depends on what arguments you pass. If no arguments are given the range is [0, \
           32767].
    If two arguments are given the range is [START, END]."
)]
pub fn random(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    match random::random(&args[1..]) {
        Ok(()) => Status::SUCCESS,
        Err(why) => Status::error(why),
    }
}

#[builtin(
    names = "true",
    desc = "does nothing sucessfully",
    man = "
SYNOPSIS
    true

DESCRIPTION
    Sets the exit status to 0."
)]
pub fn true_(args: &[types::Str], _: &mut Shell<'_>) -> Status { Status::SUCCESS }

#[builtin(
    names = "false",
    desc = "does nothing unsuccessfully",
    man = "
SYNOPSIS
    false

DESCRIPTION
    Sets the exit status to 1."
)]
pub fn false_(args: &[types::Str], _: &mut Shell<'_>) -> Status { Status::FALSE }

#[builtin(
    desc = "wait for a background job",
    man = "
SYNOPSIS
    wait

DESCRIPTION
    Wait for the background jobs to finish"
)]
pub fn wait(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if let Err(err) = shell.wait_for_background() {
        Status::error(err.to_string())
    } else {
        Status::SUCCESS
    }
}

#[builtin(
    desc = "list all jobs running in the background",
    man = "
SYNOPSIS
    jobs

DESCRIPTION
    Prints a list of all jobs running in the background."
)]
pub fn jobs(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    job_control::jobs(shell);
    Status::SUCCESS
}

#[builtin(
    desc = "sends jobs to background",
    man = "
SYNOPSIS
    bg PID

DESCRIPTION
    bg sends the job to the background resuming it if it has stopped."
)]
pub fn bg(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    job_control::bg(shell, &args[1..])
}

#[builtin(
    desc = "bring job to the foreground",
    man = "
SYNOPSIS
    fg PID

DESCRIPTION
    fg brings the specified job to foreground resuming it if it has stopped."
)]
pub fn fg(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    job_control::fg(shell, &args[1..])
}

#[builtin(
    desc = "disown processes",
    man = "
SYNOPSIS
    disown [ --help | -r | -h | -a ][PID...]

DESCRIPTION
    Disowning a process removes that process from the shell's background process table.

OPTIONS
    -r  Remove all running jobs from the background process list.
    -h  Specifies that each job supplied will not receive the SIGHUP signal when the shell \
           receives a SIGHUP.
    -a  If no job IDs were supplied, remove all jobs from the background process list."
)]
pub fn disown(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match job_control::disown(shell, &args[1..]) {
        Ok(()) => Status::SUCCESS,
        Err(err) => Status::error(format!("ion: disown: {}", err)),
    }
}

#[builtin(
    desc = "get help for builtins",
    man = "
SYNOPSIS
    help [BUILTIN]

DESCRIPTION
    Get the short description for BUILTIN. If no argument is provided, list all the builtins"
)]
pub fn help(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if let Some(command) = args.get(1) {
        if let Some(help) = shell.builtins().get_help(command) {
            println!("{}", help);
        } else {
            println!("Command helper not found [run 'help']...");
        }
    } else {
        println!("{}", shell.builtins().keys().sorted().format("\n"));
    }
    Status::SUCCESS
}

use regex::Regex;
#[builtin(
    desc = "checks if the second argument contains any proportion of the first",
    man = "
SYNOPSIS
    matches VALUE VALUE

DESCRIPTION
    Makes the exit status equal 0 if the first argument contains the second.
    Otherwise matches makes the exit status equal 1.

EXAMPLES
    Returns true:
        matches xs x
    Returns false:
        matches x xs"
)]
pub fn matches(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if args[1..].len() != 2 {
        return Status::bad_argument("match takes two arguments");
    }
    let input = &args[1];
    let re = match Regex::new(&args[2]) {
        Ok(r) => r,
        Err(e) => {
            return Status::error(format!("couldn't compile input regex {}: {}", args[2], e));
        }
    };

    if re.is_match(input) {
        Status::TRUE
    } else {
        Status::FALSE
    }
}

#[builtin(
    desc = "checks if the provided file descriptor is a tty",
    man = "
SYNOPSIS
    isatty [FD]

DESCRIPTION
    Returns 0 exit status if the supplied file descriptor is a tty."
)]
pub fn isatty(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if args.len() > 1 {
        // sys::isatty expects a usize if compiled for redox but otherwise a i32.
        let pid = args[1].parse::<i32>();

        match pid {
            Ok(r) => nix::unistd::isatty(r).unwrap().into(),
            Err(_) => Status::error("ion: isatty given bad number"),
        }
    } else {
        Status::SUCCESS
    }
}
