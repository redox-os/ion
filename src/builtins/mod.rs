pub mod source;
pub mod variables;
pub mod functions;
pub mod calc;

mod conditionals;
mod job_control;
mod test;
mod echo;
mod set;

use self::conditionals::{contains, ends_with, starts_with};
use self::echo::echo;
use self::functions::fn_;
use self::source::source;
use self::test::test;
use self::variables::{alias, drop_alias, drop_array, drop_variable};

use fnv::FnvHashMap;
use std::error::Error;
use std::io::{self, Write};

use parser::QuoteTerminator;
use shell::{self, FlowLogic, Shell, ShellHistory};
use shell::job_control::{JobControl, ProcessState};
use shell::status::*;
use sys;

/// Structure which represents a Terminal's command.
/// This command structure contains a name, and the code which run the
/// functionnality associated to this one, with zero, one or several argument(s).
pub struct Builtin {
    pub name: &'static str,
    pub help: &'static str,
    pub main: fn(&[&str], &mut Shell) -> i32,
}

impl Builtin {
    /// Return the map from command names to commands
    pub fn map() -> FnvHashMap<&'static str, Self> {
        let mut commands: FnvHashMap<&str, Self> = FnvHashMap::with_capacity_and_hasher(32, Default::default());

        /*
        Quick and clean way to insert a builtin, define a function named as the builtin
        for example:
        fn builtin_not (args: &[&str], shell: &mut Shell) -> i32 {
            let cmd = args[1..].join(" ");
            shell.on_command(&cmd);
            match shell.previous_status {
                SUCCESS => FAILURE,
                FAILURE => SUCCESS,
                _ => shell.previous_status
            }
        }

        insert_builtin!("not", builtin_not, "Reverses the exit status value of the given command.");
        */

        macro_rules! insert_builtin {
            ($name:expr, $func:ident, $help:expr) => {
                commands.insert(
                    $name,
                    Builtin {
                        name: $name,
                        help: $help,
                        main: $func,
                    }
                );
            }
        }

        /* Directories */
        insert_builtin!("cd", builtin_cd, "Change the current directory\n    cd <path>");

        insert_builtin!("dirs", builtin_dirs, "Display the current directory stack");
        insert_builtin!("pushd", builtin_pushd, "Push a directory to the stack");
        insert_builtin!("popd", builtin_popd, "Pop a directory from the stack");

        /* Aliases */
        insert_builtin!("alias", builtin_alias, "View, set or unset aliases");
        insert_builtin!("unalias", builtin_unalias, "Delete an alias");

        /* Variables */
        insert_builtin!("fn", builtin_fn, "Print list of functions");
        insert_builtin!("read", builtin_read, "Read some variables\n    read <variable>");
        insert_builtin!("drop", builtin_drop, "Delete a variable");

        /* Misc */
        insert_builtin!("matches", builtin_matches, "Checks if a string matches a given regex");
        insert_builtin!("not", builtin_not, "Reverses the exit status value of the given command.");
        insert_builtin!(
            "set",
            builtin_set,
            "Set or unset values of shell options and positional parameters."
        );
        insert_builtin!("eval", builtin_eval, "evaluates the evaluated expression");
        insert_builtin!("exit", builtin_exit, "Exits the current session");
        insert_builtin!(
            "wait",
            builtin_wait,
            "Waits until all running background processes have completed"
        );
        insert_builtin!("jobs", builtin_jobs, "Displays all jobs that are attached to the background");
        insert_builtin!("bg", builtin_bg, "Resumes a stopped background process");
        insert_builtin!("fg", builtin_fg, "Resumes and sets a background process as the active process");
        insert_builtin!("suspend", builtin_suspend, "Suspends the shell with a SIGTSTOP signal");
        insert_builtin!(
            "disown",
            builtin_disown,
            "Disowning a process removes that process from the shell's background process table."
        );
        insert_builtin!("history", builtin_history, "Display a log of all commands previously executed");
        insert_builtin!(
            "source",
            builtin_source,
            "Evaluate the file following the command or re-initialize the init file"
        );
        insert_builtin!("echo", builtin_echo, "Display a line of text");
        insert_builtin!("test", builtin_test, "Performs tests on files and text");
        insert_builtin!("calc", builtin_calc, "Calculate a mathematical expression");
        insert_builtin!("true", builtin_true, "Do nothing, successfully");
        insert_builtin!("false", builtin_false, "Do nothing, unsuccessfully");
        insert_builtin!(
            "help",
            builtin_help,
            "Display helpful information about a given command or list commands if none specified\n    help <command>"
        );
        insert_builtin!(
            "and",
            builtin_and,
            "Execute the command if the shell's previous status is success"
        );
        insert_builtin!(
            "or",
            builtin_or,
            "Execute the command if the shell's previous status is failure"
        );
        insert_builtin!(
            "starts-with",
            starts_with,
            "Evaluates if the supplied argument starts with a given string"
        );
        insert_builtin!(
            "ends-with",
            ends_with,
            "Evaluates if the supplied argument ends with a given string"
        );
        insert_builtin!(
            "contains",
            contains,
            "Evaluates if the supplied argument contains a given string"
        );

        commands
    }
}

/* Definitions of simple builtins go here */

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

fn builtin_unalias(args: &[&str], shell: &mut Shell) -> i32 { drop_alias(&mut shell.variables, args) }

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

fn builtin_disown(args: &[&str], shell: &mut Shell) -> i32 { job_control::disown(shell, &args[1..]) }

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
        let mut commands = builtins.keys().cloned().collect::<Vec<&str>>();
        commands.sort();

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
    shell.exit(
        args.get(1)
            .and_then(|status| status.parse::<i32>().ok())
            .unwrap_or(previous_status),
    )
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
            let _ = stderr.write_all(format!("couldn't compile input regex {}: {}\n", args[2], e).as_bytes());
            return FAILURE;
        }
    };

    if re.is_match(input) { SUCCESS } else { FAILURE }
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
