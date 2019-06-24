pub mod man_pages;

mod calc;
mod command_info;
mod conditionals;
mod echo;
mod exists;
mod functions;
mod helpers;
mod is;
mod job_control;
mod random;
mod set;
mod source;
mod status;
mod test;
mod variables;

use self::{
    command_info::{find_type, which},
    echo::echo,
    exists::exists,
    functions::print_functions,
    is::is,
    man_pages::*,
    source::source,
    status::status,
    test::test,
    variables::{alias, drop_alias, drop_array, drop_variable},
};
pub use self::{helpers::Status, man_pages::check_help};
use crate::{
    shell::{sys, Capture, Shell, Value},
    types,
};
use hashbrown::HashMap;
use itertools::Itertools;
use liner::{Completer, Context};
use std::{
    borrow::Cow,
    io::{self, BufRead},
    path::PathBuf,
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
    match arg.chars().nth(0) {
        Some('+') => Some(true),
        Some('-') => Some(false),
        _ => None,
    }
    .and_then(|b| arg[1..].parse::<usize>().ok().map(|num| (b, num)))
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
    pub fn keys(&self) -> impl Iterator<Item = &str> { self.fcts.keys().cloned() }

    /// Get the provided help for a given builtin
    pub fn get_help(&self, func: &str) -> Option<&str> { self.help.get(func).cloned() }

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
        self.add("fn", &builtin_fn, "Print list of functions")
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
    /// Contains `which`, `test`, `exists`, `popd`, `pushd`, `dirs`, `cd`
    pub fn with_files_and_directory(&mut self) -> &mut Self {
        self.add("which", &builtin_which, "Shows the full path of commands")
            .add("test", &builtin_test, "Performs tests on files and text")
            .add("exists", &builtin_exists, "Performs tests on files and text")
            .add("popd", &builtin_popd, "Pop a directory from the stack")
            .add("pushd", &builtin_pushd, "Push a directory to the stack")
            .add("dirs", &builtin_dirs, "Display the current directory stack")
            .add("cd", &builtin_cd, "Change the current directory\n    cd <path>")
            .add("dir_depth", &builtin_dir_depth, "Set the maximum directory depth")
    }

    /// Utilities to test values
    ///
    /// Contains `bool`, `calc`, `eq`, `is`, `true`, `false`, `starts-with`, `ends-with`,
    /// `contains`, `matches`, `random`
    pub fn with_values_tests(&mut self) -> &mut Self {
        self.add("bool", &builtin_bool, "If the value is '1' or 'true', return 0 exit status")
            .add("calc", &builtin_calc, "Calculate a mathematical expression")
            .add("eq", &builtin_eq, "Simple alternative to == and !=")
            .add("is", &builtin_is, "Simple alternative to == and !=")
            .add("true", &builtin_true, "Do nothing, successfully")
            .add("false", &builtin_false, "Do nothing, unsuccessfully")
            .add(
                "starts-with",
                &starts_with,
                "Evaluates if the supplied argument starts with a given string",
            )
            .add(
                "ends-with",
                &ends_with,
                "Evaluates if the supplied argument ends with a given string",
            )
            .add(
                "contains",
                &contains,
                "Evaluates if the supplied argument contains a given string",
            )
            .add("matches", &builtin_matches, "Checks if a string matches a given regex")
            .add("random", &builtin_random, "Outputs a random u64")
    }

    /// Basic utilities for any ion embedded library
    ///
    /// Contains `help`, `source`, `status`, `echo`, `type`
    pub fn with_basic(&mut self) -> &mut Self {
        self.add("help", &builtin_help, HELP_DESC)
            .add("source", &builtin_source, SOURCE_DESC)
            .add("status", &builtin_status, "Evaluates the current runtime status")
            .add("echo", &builtin_echo, "Display a line of text")
            .add("type", &builtin_type, "indicates how a command would be interpreted")
    }

    /// Utilities specific for a shell, that should probably not be included in an embedded context
    ///
    /// Contains `eval`, `exec`, `exit`, `set`, `suspend`
    pub fn with_shell_unsafe(&mut self) -> &mut Self {
        self.add("eval", &builtin_eval, "Evaluates the evaluated expression")
            .add(
                "set",
                &builtin_set,
                "Set or unset values of shell options and positional parameters.",
            )
            .add("suspend", &builtin_suspend, "Suspends the shell with a SIGTSTOP signal")
    }
}

fn starts_with(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    Status::from_exit_code(conditionals::starts_with(args))
}
fn ends_with(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    Status::from_exit_code(conditionals::ends_with(args))
}
fn contains(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    Status::from_exit_code(conditionals::contains(args))
}

// Definitions of simple builtins go here
pub fn builtin_status(args: &[types::Str], shell: &mut Shell<'_>) -> Status { status(args, shell) }

pub fn builtin_dir_depth(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
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

pub fn builtin_cd(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_CD) {
        return Status::SUCCESS;
    }

    let err = match args.get(1) {
        Some(dir) => {
            let dir = dir.as_ref();
            if let Some(Value::Array(cdpath)) = shell.variables().get("CDPATH").cloned() {
                if dir == "-" {
                    shell.dir_stack_mut().switch_to_previous_directory()
                } else {
                    let check_cdpath_first = cdpath
                        .iter()
                        .map(|path| {
                            let path_dir = format!("{}/{}", path, dir);
                            shell.dir_stack_mut().change_and_push_dir(&path_dir)
                        })
                        .find(Result::is_ok)
                        .unwrap_or_else(|| shell.dir_stack_mut().change_and_push_dir(dir));
                    shell.dir_stack_mut().popd(1);
                    check_cdpath_first
                }
            } else {
                shell.dir_stack_mut().change_and_push_dir(dir)
            }
        }
        None => shell.dir_stack_mut().switch_to_home_directory(),
    };

    match err {
        Ok(()) => {
            let _ = shell.fork_function(Capture::None, |_| Ok(()), "CD_CHANGE", &["ion"]);
            Status::SUCCESS
        }
        Err(why) => Status::error(format!("{}", why)),
    }
}

pub fn builtin_bool(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if args.len() != 2 {
        return Status::error("bool requires one argument");
    }

    let opt = if args[1].is_empty() { None } else { shell.variables().get_str(&args[1][1..]).ok() };

    match opt.as_ref().map(types::Str::as_str) {
        Some("1") => (),
        Some("true") => (),
        _ => match &*args[1] {
            "1" => (),
            "true" => (),
            "--help" => println!("{}", MAN_BOOL),
            "-h" => println!("{}", MAN_BOOL),
            _ => return Status::from_exit_code(1),
        },
    }
    Status::SUCCESS
}

pub fn builtin_is(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_IS) {
        return Status::SUCCESS;
    }

    is(args, shell)
}

pub fn builtin_dirs(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    // converts pbuf to an absolute path if possible
    fn try_abs_path(pbuf: &PathBuf) -> Cow<'_, str> {
        Cow::Owned(
            pbuf.canonicalize().unwrap_or_else(|_| pbuf.clone()).to_string_lossy().to_string(),
        )
    }

    if check_help(args, MAN_DIRS) {
        return Status::SUCCESS;
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

    let mut iter = shell.dir_stack().dirs().enumerate().map(mapper);

    if let Some(arg) = num_arg {
        let num = match parse_numeric_arg(arg.as_ref()) {
            Some((true, num)) => num,
            Some((false, num)) if shell.dir_stack().dirs().count() > num => {
                shell.dir_stack().dirs().count() - num - 1
            }
            _ => return Status::error(format!("ion: dirs: {}: invalid argument", arg)),
        };
        match iter.nth(num) {
            Some(x) => {
                println!("{}", x);
                Status::SUCCESS
            }
            None => Status::error(""),
        }
    } else {
        println!("{}", iter.join(if multiline { "\n" } else { " " }));
        Status::SUCCESS
    }
}

pub fn builtin_pushd(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_PUSHD) {
        return Status::SUCCESS;
    }

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
            if let Err(why) = shell.dir_stack_mut().pushd(dir, keep_front) {
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
            .join(" ")
    );
    Status::SUCCESS
}

pub fn builtin_popd(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_POPD) {
        return Status::SUCCESS;
    }

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
        } else {
            let (count_from_front, num) = match parse_numeric_arg(arg) {
                Some(n) => n,
                None => {
                    return Status::error(format!("ion: popd: {}: invalid argument", arg));
                }
            };

            index = if count_from_front {
                // <=> input number is positive
                num
            } else if let Some(n) = (len - 1).checked_sub(num) {
                n
            } else {
                return Status::error("ion: popd: negative directory stack index out of range");
            };
        }
    }

    // apply -n
    if index == 0 && keep_front {
        index = 1;
    } else if index == 0 {
        // change to new directory, return if not possible
        if let Err(why) = shell.dir_stack_mut().set_current_dir_by_index(1) {
            return Status::error(format!("ion: popd: {}", why));
        }
    }

    // pop element
    if shell.dir_stack_mut().popd(index).is_some() {
        println!(
            "{}",
            shell
                .dir_stack()
                .dirs()
                .map(|dir| dir.to_str().unwrap_or("ion: no directory found"))
                .join(" ")
        );
        Status::SUCCESS
    } else {
        Status::error(format!("ion: popd: {}: directory stack index out of range", index))
    }
}

pub fn builtin_alias(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let args_str = args[1..].join(" ");
    alias(shell.variables_mut(), &args_str)
}

pub fn builtin_unalias(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    drop_alias(shell.variables_mut(), args)
}

// TODO There is a man page for fn however the -h and --help flags are not
// checked for.
pub fn builtin_fn(_: &[types::Str], shell: &mut Shell<'_>) -> Status {
    print_functions(shell.variables())
}

struct EmptyCompleter;

impl Completer for EmptyCompleter {
    fn completions(&mut self, _start: &str) -> Vec<String> { Vec::new() }
}

pub fn builtin_read(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_READ) {
        return Status::SUCCESS;
    }

    if atty::is(atty::Stream::Stdin) {
        let mut con = Context::new();
        for arg in args.iter().skip(1) {
            match con.read_line(format!("{}=", arg.trim()), None, &mut EmptyCompleter) {
                Ok(buffer) => {
                    shell.variables_mut().set(arg.as_ref(), buffer.trim());
                }
                Err(_) => return Status::error(""),
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

pub fn builtin_drop(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_DROP) {
        return Status::SUCCESS;
    }
    if args.len() >= 2 && args[1] == "-a" {
        drop_array(shell.variables_mut(), args)
    } else {
        drop_variable(shell.variables_mut(), args)
    }
}

pub fn builtin_set(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_SET) {
        return Status::SUCCESS;
    }
    set::set(args, shell)
}

pub fn builtin_eq(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_EQ) {
        return Status::SUCCESS;
    }

    is(args, shell)
}

pub fn builtin_eval(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_EVAL) {
        Status::SUCCESS
    } else {
        shell.execute_command(args[1..].join(" ").as_bytes()).unwrap_or_else(|_| {
            Status::error("ion: supplied eval expression was not terminated".to_string())
        })
    }
}

pub fn builtin_source(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_SOURCE) {
        return Status::SUCCESS;
    }
    source(shell, args)
}

pub fn builtin_echo(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_ECHO) {
        return Status::SUCCESS;
    }
    match echo(args) {
        Ok(()) => Status::SUCCESS,
        Err(why) => Status::error(why.to_string()),
    }
}

pub fn builtin_test(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    // Do not use `check_help` for the `test` builtin. The
    // `test` builtin contains a "-h" option.
    match test(args) {
        Ok(true) => Status::SUCCESS,
        Ok(false) => Status::error(""),
        Err(why) => Status::error(why),
    }
}

// TODO create manpage.
pub fn builtin_calc(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    match calc::calc(&args[1..]) {
        Ok(()) => Status::SUCCESS,
        Err(why) => Status::error(why),
    }
}

pub fn builtin_random(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_RANDOM) {
        return Status::SUCCESS;
    }
    match random::random(&args[1..]) {
        Ok(()) => Status::SUCCESS,
        Err(why) => Status::error(why),
    }
}

pub fn builtin_true(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    check_help(args, MAN_TRUE);
    Status::SUCCESS
}

pub fn builtin_false(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_FALSE) {
        return Status::SUCCESS;
    }
    Status::error("")
}

// TODO create a manpage
pub fn builtin_wait(_: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let _ = shell.wait_for_background();
    Status::SUCCESS
}

pub fn builtin_jobs(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    check_help(args, MAN_JOBS);
    job_control::jobs(shell);
    Status::SUCCESS
}

pub fn builtin_bg(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_BG) {
        return Status::SUCCESS;
    }
    job_control::bg(shell, &args[1..])
}

pub fn builtin_fg(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_FG) {
        return Status::SUCCESS;
    }
    job_control::fg(shell, &args[1..])
}

pub fn builtin_suspend(args: &[types::Str], _shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_SUSPEND) {
        return Status::SUCCESS;
    }
    let _ = unsafe { libc::kill(0, libc::SIGSTOP) };
    Status::SUCCESS
}

pub fn builtin_disown(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    for arg in args {
        if *arg == "--help" {
            println!("{}", MAN_DISOWN);
            return Status::SUCCESS;
        }
    }
    match job_control::disown(shell, &args[1..]) {
        Ok(()) => Status::SUCCESS,
        Err(err) => Status::error(format!("ion: disown: {}", err)),
    }
}

pub fn builtin_help(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if let Some(command) = args.get(1) {
        if let Some(help) = shell.builtins().get_help(command) {
            println!("{}", help);
        } else {
            println!("Command helper not found [run 'help']...");
        }
    } else {
        println!("{}", shell.builtins().keys().join(""));
    }
    Status::SUCCESS
}

use regex::Regex;
pub fn builtin_matches(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_MATCHES) {
        return Status::SUCCESS;
    }
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
        Status::SUCCESS
    } else {
        Status::error("")
    }
}

pub fn builtin_exists(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_EXISTS) {
        return Status::SUCCESS;
    }
    match exists(args, shell) {
        Ok(true) => Status::SUCCESS,
        Ok(false) => Status::error(""),
        Err(why) => Status::error(why),
    }
}

pub fn builtin_which(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match which(args, shell) {
        Ok(result) => result,
        Err(()) => Status::error(""),
    }
}

pub fn builtin_type(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match find_type(args, shell) {
        Ok(result) => result,
        Err(()) => Status::error(""),
    }
}

pub fn builtin_isatty(args: &[types::Str], _: &mut Shell<'_>) -> Status {
    if check_help(args, MAN_ISATTY) {
        return Status::SUCCESS;
    }

    if args.len() > 1 {
        // sys::isatty expects a usize if compiled for redox but otherwise a i32.
        #[cfg(target_os = "redox")]
        let pid = args[1].parse::<usize>();
        #[cfg(not(target_os = "redox"))]
        let pid = args[1].parse::<i32>();

        match pid {
            Ok(r) => {
                if sys::isatty(r) {
                    Status::SUCCESS
                } else {
                    Status::error("")
                }
            }
            Err(_) => Status::error("ion: isatty given bad number"),
        }
    } else {
        Status::SUCCESS
    }
}
