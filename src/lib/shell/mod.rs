mod assignments;
pub(crate) mod binary;
pub(crate) mod colors;
mod completer;
pub(crate) mod directory_stack;
pub(crate) mod escape;
mod flow;
pub(crate) mod flow_control;
mod fork;
pub mod fork_function;
mod history;
mod job;
pub(crate) mod pipe_exec;
pub(crate) mod signals;
pub mod status;
pub mod variables;

pub mod flags {
    /// Exit from the shell on the first error.
    pub const ERR_EXIT: u8 = 1;
    /// Print commands that are to be executed.
    pub const PRINT_COMMS: u8 = 2;
    /// Do not execute any commands given to the shell.
    pub const NO_EXEC: u8 = 4;
    /// Hangup on exiting the shell.
    pub const HUPONEXIT: u8 = 8;
    /// Used by an interactive session to know when the input is not terminated.
    pub const UNTERMINATED: u8 = 16;
}

pub use self::{
    binary::Binary,
    fork::{Capture, Fork, IonResult},
};
pub(crate) use self::{
    flow::FlowLogic,
    history::{IgnoreSetting, ShellHistory},
    job::{Job, JobKind},
    pipe_exec::{foreground, job_control},
};

use self::{
    assignments::{math, parse},
    directory_stack::DirectoryStack,
    flags::*,
    flow_control::{FlowControl, Function, FunctionError},
    foreground::ForegroundSignals,
    job_control::{BackgroundProcess, JobControl},
    pipe_exec::PipelineExecution,
    status::*,
    variables::{GetVariable, Value, Variables},
};
use crate::{
    builtins::{BuiltinMap, BUILTINS},
    lexers::{Key, Operator, Primitive},
    parser::{assignments::value_check, pipelines::Pipeline, Expander, Select, Terminator},
    sys,
    types::{self, Array},
};
use itertools::Itertools;
use liner::Context;
use std::{
    fs,
    io::{self, Read, Write},
    iter::FromIterator,
    ops::Deref,
    path::Path,
    process,
    sync::{atomic::Ordering, Arc, Mutex},
    time::SystemTime,
};
use xdg::BaseDirectories;

#[derive(Debug, Fail)]
pub enum IonError {
    #[fail(display = "failed to fork: {}", why)]
    Fork { why: io::Error },
    #[fail(display = "element does not exist")]
    DoesNotExist,
    #[fail(display = "input was not terminated")]
    Unterminated,
    #[fail(display = "function error: {}", why)]
    Function { why: FunctionError },
}

/// The shell structure is a megastructure that manages all of the state of the shell throughout
/// the entirety of the
/// program. It is initialized at the beginning of the program, and lives until the end of the
/// program.
pub struct Shell {
    /// Contains a list of built-in commands that were created when the program
    /// started.
    pub(crate) builtins: &'static BuiltinMap,
    /// Contains the history, completions, and manages writes to the history file.
    /// Note that the context is only available in an interactive session.
    pub(crate) context: Option<Context>,
    /// Contains the aliases, strings, and array variable maps.
    pub variables: Variables,
    /// Contains the current state of flow control parameters.
    flow_control: FlowControl,
    /// Contains the directory stack parameters.
    pub(crate) directory_stack: DirectoryStack,
    /// When a command is executed, the final result of that command is stored
    /// here.
    pub previous_status: i32,
    /// The job ID of the previous command sent to the background.
    pub(crate) previous_job: u32,
    /// Contains all the boolean flags that control shell behavior.
    pub flags: u8,
    /// Contains information on all of the active background processes that are being managed
    /// by the shell.
    pub(crate) background: Arc<Mutex<Vec<BackgroundProcess>>>,
    /// If set, denotes that this shell is running as a background job.
    pub(crate) is_background_shell: bool,
    /// Set when a signal is received, this will tell the flow control logic to
    /// abort.
    pub(crate) break_flow: bool,
    // Useful for disabling the execution of the `tcsetpgrp` call.
    pub(crate) is_library: bool,
    /// When the `fg` command is run, this will be used to communicate with the specified
    /// background process.
    foreground_signals: Arc<ForegroundSignals>,
    /// Stores the patterns used to determine whether a command should be saved in the history
    /// or not
    ignore_setting: IgnoreSetting,
}

#[derive(Default)]
pub struct ShellBuilder;

impl ShellBuilder {
    pub fn as_binary(&self) -> Shell { Shell::new(false) }

    pub fn as_library(&self) -> Shell { Shell::new(true) }

    pub fn set_unique_pid(self) -> ShellBuilder {
        if let Ok(pid) = sys::getpid() {
            if sys::setpgid(0, pid).is_ok() {
                let _ = sys::tcsetpgrp(0, pid);
            }
        }

        self
    }

    pub fn block_signals(self) -> ShellBuilder {
        // This will block SIGTSTP, SIGTTOU, SIGTTIN, and SIGCHLD, which is required
        // for this shell to manage its own process group / children / etc.
        signals::block();

        self
    }

    pub fn install_signal_handler(self) -> ShellBuilder {
        extern "C" fn handler(signal: i32) {
            let signal = match signal {
                sys::SIGINT => signals::SIGINT,
                sys::SIGHUP => signals::SIGHUP,
                sys::SIGTERM => signals::SIGTERM,
                _ => unreachable!(),
            };

            signals::PENDING.store(signal as usize, Ordering::SeqCst);
        }

        let _ = sys::signal(sys::SIGHUP, handler);
        let _ = sys::signal(sys::SIGINT, handler);
        let _ = sys::signal(sys::SIGTERM, handler);

        extern "C" fn sigpipe_handler(signal: i32) {
            let _ = io::stdout().flush();
            let _ = io::stderr().flush();
            sys::fork_exit(127 + signal);
        }

        let _ = sys::signal(sys::SIGPIPE, sigpipe_handler);

        self
    }

    pub fn new() -> ShellBuilder { ShellBuilder }
}

impl Shell {
    /// A method for capturing the output of the shell, and performing actions without modifying
    /// the state of the original shell. This performs a fork, taking a closure that controls
    /// the shell in the child of the fork.
    ///
    /// The method is non-blocking, and therefore will immediately return file handles to the
    /// stdout and stderr of the child. The PID of the child is returned, which may be used to
    /// wait for and obtain the exit status.
    pub fn fork<F: FnMut(&mut Shell)>(
        &self,
        capture: Capture,
        child_func: F,
    ) -> Result<IonResult, IonError> {
        Fork::new(self, capture).exec(child_func)
    }

    /// A method for executing a function with the given `name`, using `args` as the input.
    /// If the function does not exist, an `IonError::DoesNotExist` is returned.
    pub fn execute_function<S: AsRef<str>>(
        &mut self,
        name: &str,
        args: &[S],
    ) -> Result<i32, IonError> {
        self.variables.get::<Function>(name).ok_or(IonError::DoesNotExist).and_then(|function| {
            function
                .execute(self, args)
                .map(|_| self.previous_status)
                .map_err(|err| IonError::Function { why: err })
        })
    }

    /// A method for executing scripts in the Ion shell without capturing. Given a `Path`, this
    /// method will attempt to execute that file as a script, and then returns the final exit
    /// status of the evaluated script.
    pub fn execute_file<P: AsRef<Path>>(&mut self, script: P) {
        match fs::read_to_string(script.as_ref()) {
            Ok(script) => {
                if self.terminate_script_quotes(script.bytes()) == FAILURE {
                    self.previous_status = FAILURE;
                }
            }
            Err(err) => eprintln!("ion: {}", err),
        }
    }

    /// A method for executing commands in the Ion shell without capturing. It takes command(s)
    /// as
    /// a string argument, parses them, and executes them the same as it would if you had
    /// executed
    /// the command(s) in the command line REPL interface for Ion. If the supplied command is
    /// not
    /// terminated, then an error will be returned.
    pub fn execute_command<'a, T>(&mut self, command: &T) -> Result<i32, IonError>
    where
        T: 'a + AsRef<str> + std::clone::Clone + std::convert::From<&'a str>,
    {
        for cmd in command.as_ref().bytes().batching(|bytes| Terminator::new(bytes).terminate()) {
            match cmd {
                Ok(stmt) => self.on_command(&stmt),
                Err(_) => return Err(IonError::Unterminated),
            }
        }
        Ok(self.previous_status)
    }

    /// Obtains a variable, returning an empty string if it does not exist.
    pub(crate) fn get_str_or_empty(&self, name: &str) -> types::Str {
        self.variables.get_str_or_empty(name)
    }

    /// Gets any variable, if it exists within the shell's variable map.
    pub fn get<T>(&self, name: &str) -> Option<T>
    where
        Variables: GetVariable<T>,
    {
        self.variables.get::<T>(name)
    }

    /// Sets a variable of `name` with the given `value` in the shell's variable map.
    pub fn set<T: Into<Value>>(&mut self, name: &str, value: T) { self.variables.set(name, value); }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    pub(crate) fn run_pipeline(&mut self, pipeline: &mut Pipeline) -> Option<i32> {
        let command_start_time = SystemTime::now();

        // Branch if -> input == shell command i.e. echo
        let exit_status = if let Some(main) = pipeline.items[0].job.builtin {
            pipeline.expand(self);
            // Run the 'main' of the command and set exit_status
            if !pipeline.requires_piping() {
                if self.flags & PRINT_COMMS != 0 {
                    eprintln!("> {}", pipeline.to_string());
                }
                if self.flags & NO_EXEC != 0 {
                    Some(SUCCESS)
                } else {
                    let borrowed = &pipeline.items[0].job.args;
                    Some(main(borrowed, self))
                }
            } else {
                Some(self.execute_pipeline(pipeline))
            }
        // Branch else if -> input == shell function and set the exit_status
        } else if let Some(function) =
            self.variables.get::<Function>(&pipeline.items[0].job.command)
        {
            if !pipeline.requires_piping() {
                let args = pipeline.items[0].job.args.deref();
                match function.execute(self, args) {
                    Ok(()) => None,
                    Err(FunctionError::InvalidArgumentCount) => {
                        eprintln!("ion: invalid number of function arguments supplied");
                        Some(FAILURE)
                    }
                    Err(FunctionError::InvalidArgumentType(expected_type, value)) => {
                        eprintln!(
                            "ion: function argument has invalid type: expected {}, found value \
                             \'{}\'",
                            expected_type, value
                        );
                        Some(FAILURE)
                    }
                }
            } else {
                Some(self.execute_pipeline(pipeline))
            }
        } else {
            pipeline.expand(self);
            Some(self.execute_pipeline(pipeline))
        };

        // If `RECORD_SUMMARY` is set to "1" (True, Yes), then write a summary of the
        // pipline just executed to the the file and context histories. At the
        // moment, this means record how long it took.
        if let Some(context) = self.context.as_mut() {
            if "1" == &*self.variables.get_str_or_empty("RECORD_SUMMARY") {
                if let Ok(elapsed_time) = command_start_time.elapsed() {
                    let summary = format!(
                        "#summary# elapsed real time: {}.{:09} seconds",
                        elapsed_time.as_secs(),
                        elapsed_time.subsec_nanos()
                    );
                    context.history.push(summary.into()).unwrap_or_else(|err| {
                        eprintln!("ion: history append: {}", err);
                    });
                }
            }
        }

        // Retrieve the exit_status and set the $? variable and history.previous_status
        if let Some(code) = exit_status {
            self.set("?", code.to_string());
            self.previous_status = code;
        }

        exit_status
    }

    /// Evaluates the source init file in the user's home directory.
    pub fn evaluate_init_file(&mut self) {
        let base_dirs = match BaseDirectories::with_prefix("ion") {
            Ok(base_dirs) => base_dirs,
            Err(err) => {
                eprintln!("ion: unable to get base directory: {}", err);
                return;
            }
        };
        match base_dirs.find_config_file("initrc") {
            Some(initrc) => self.execute_file(&initrc),
            None => {
                if let Err(err) = base_dirs.place_config_file("initrc") {
                    eprintln!("ion: could not create initrc file: {}", err);
                }
            }
        }
    }

    /// Cleanly exit ion
    pub fn exit(&mut self, status: i32) -> ! {
        self.prep_for_exit();
        process::exit(status);
    }

    pub(crate) fn prep_for_exit(&mut self) {
        // The context has two purposes: if it exists, this is an interactive shell; and the
        // context will also be sent a signal to commit all changes to the history file,
        // and waiting for the history thread in the background to finish.
        if self.context.is_some() {
            if self.flags & HUPONEXIT != 0 {
                self.resume_stopped();
                self.background_send(sys::SIGHUP);
            }
            self.context.as_mut().unwrap().history.commit_to_file();
        }
    }

    pub(crate) fn new(is_library: bool) -> Shell {
        let mut shell = Shell {
            builtins: BUILTINS,
            context: None,
            variables: Variables::default(),
            flow_control: FlowControl::default(),
            directory_stack: DirectoryStack::new(),
            previous_job: !0,
            previous_status: 0,
            flags: 0,
            background: Arc::new(Mutex::new(Vec::new())),
            is_background_shell: false,
            is_library,
            break_flow: false,
            foreground_signals: Arc::new(ForegroundSignals::new()),
            ignore_setting: IgnoreSetting::default(),
        };
        let ignore_patterns = shell.variables.get("HISTORY_IGNORE").unwrap();
        shell.update_ignore_patterns(&ignore_patterns);
        shell
    }

    pub fn assign(&mut self, key: &Key, value: Value) -> Result<(), String> {
        match (&key.kind, &value) {
            (Primitive::Indexed(ref index_name, ref index_kind), Value::Str(_)) => {
                let index = value_check(self, index_name, index_kind)
                    .map_err(|why| format!("{}: {}", key.name, why))?;

                match index {
                    Value::Str(ref index) => {
                        let lhs = self
                            .variables
                            .get_mut(key.name)
                            .ok_or_else(|| "index value does not exist".to_string())?;

                        match lhs {
                            Value::HashMap(hmap) => {
                                let _ = hmap.insert(index.clone(), value);
                                Ok(())
                            }
                            Value::BTreeMap(bmap) => {
                                let _ = bmap.insert(index.clone(), value);
                                Ok(())
                            }
                            Value::Array(array) => {
                                let index_num = index.parse::<usize>().map_err(|_| {
                                    format!("index variable is not a numeric value: `{}`", index)
                                })?;

                                if let (Some(var), Value::Str(val)) =
                                    (array.get_mut(index_num), value)
                                {
                                    *var = val;
                                }
                                Ok(())
                            }
                            _ => Ok(()),
                        }
                    }
                    Value::Array(_) => Err("index variable cannot be an array".into()),
                    Value::HashMap(_) => Err("index variable cannot be a hmap".into()),
                    Value::BTreeMap(_) => Err("index variable cannot be a bmap".into()),
                    _ => Ok(()),
                }
            }
            (_, Value::Str(_))
            | (_, Value::Array(_))
            | (Primitive::HashMap(_), Value::HashMap(_))
            | (Primitive::BTreeMap(_), Value::BTreeMap(_)) => {
                self.variables.set(key.name, value);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn overwrite(&mut self, key: &Key, operator: Operator, rhs: Value) -> Result<(), String> {
        let lhs = self
            .variables
            .get_mut(key.name)
            .ok_or_else(|| format!("cannot update non existing variable `{}`", key.name))?;

        match lhs {
            Value::Str(lhs) => {
                if let Value::Str(rhs) = rhs {
                    match operator {
                        Operator::Concatenate => lhs.push_str(&rhs),
                        Operator::ConcatenateHead => {
                            *lhs = rhs + lhs;
                        }
                        _ => {
                            let action =
                                math(&key.kind, operator, &rhs).map_err(|why| why.to_string())?;
                            let value = parse(&lhs, &*action).map_err(|why| why.to_string())?;
                            *lhs = value.into();
                        }
                    }
                }
                Ok(())
            }
            Value::Array(array) => match rhs {
                Value::Str(rhs) => match operator {
                    Operator::Concatenate => {
                        array.push(rhs.clone());
                        Ok(())
                    }
                    Operator::ConcatenateHead => {
                        array.insert(0, rhs.clone());
                        Ok(())
                    }
                    Operator::Filter => {
                        array.retain(|item| item != &rhs);
                        Ok(())
                    }
                    _ => math(&Primitive::Float, operator, &rhs)
                        .and_then(|action| {
                            array
                                .iter_mut()
                                .map(|el| parse(el, &*action).map(|result| *el = result.into()))
                                .find(|e| e.is_err())
                                .unwrap_or(Ok(()))
                        })
                        .map_err(|why| why.to_string()),
                },
                Value::Array(values) => {
                    match operator {
                        Operator::Concatenate => array.extend(values.clone()),
                        Operator::ConcatenateHead => values
                            .into_iter()
                            .rev()
                            .for_each(|value| array.insert(0, value.clone())),
                        Operator::Filter => array.retain(|item| !values.contains(item)),
                        _ => {}
                    }
                    Ok(())
                }
                _ => Ok(()),
            },
            _ => {
                if let Value::Str(_) = rhs {
                    Err("type does not support this operator".to_string())
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl<'a> Expander for Shell {
    /// Uses a subshell to expand a given command.
    fn command(&self, command: &str) -> Option<types::Str> {
        let mut output = None;
        match self.fork(Capture::StdoutThenIgnoreStderr, move |shell| shell.on_command(command)) {
            Ok(result) => {
                let mut string = String::with_capacity(1024);
                match result.stdout.unwrap().read_to_string(&mut string) {
                    Ok(_) => output = Some(string),
                    Err(why) => {
                        eprintln!("ion: error reading stdout of child: {}", why);
                    }
                }
            }
            Err(why) => {
                eprintln!("ion: fork error: {}", why);
            }
        }

        // Ensure that the parent retains ownership of the terminal before exiting.
        let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
        output.map(|s| s.into())
    }

    /// Expand a string variable given if its quoted / unquoted
    fn string(&self, name: &str, quoted: bool) -> Option<types::Str> {
        use crate::ascii_helpers::AsciiReplace;
        if name == "?" {
            Some(types::Str::from(self.previous_status.to_string()))
        } else if quoted {
            self.get::<types::Str>(name)
        } else {
            self.get::<types::Str>(name).map(|x| x.ascii_replace('\n', ' '))
        }
    }

    /// Expand an array variable with some selection
    fn array(&self, name: &str, selection: Select) -> Option<types::Array> {
        if let Some(array) = self.variables.get::<types::Array>(name) {
            match selection {
                Select::All => return Some(array.clone()),
                Select::Index(id) => {
                    return id
                        .resolve(array.len())
                        .and_then(|n| array.get(n))
                        .map(|x| types::Array::from_iter(Some(x.to_owned())));
                }
                Select::Range(range) => {
                    if let Some((start, length)) = range.bounds(array.len()) {
                        if array.len() > start {
                            return Some(
                                array
                                    .iter()
                                    .skip(start)
                                    .take(length)
                                    .map(|x| x.to_owned())
                                    .collect::<types::Array>(),
                            );
                        }
                    }
                }
                _ => (),
            }
        } else if let Some(hmap) = self.variables.get::<types::HashMap>(name) {
            match selection {
                Select::All => {
                    let mut array = types::Array::new();
                    for (key, value) in hmap.iter() {
                        array.push(key.clone());
                        let f = format!("{}", value);
                        match *value {
                            Value::Str(_) => array.push(f.into()),
                            Value::Array(_) | Value::HashMap(_) | Value::BTreeMap(_) => {
                                for split in f.split_whitespace() {
                                    array.push(split.into());
                                }
                            }
                            _ => (),
                        }
                    }
                    return Some(array);
                }
                Select::Key(key) => {
                    return Some(array![format!(
                        "{}",
                        hmap.get(&*key).unwrap_or(&Value::Str("".into()))
                    )]);
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    return Some(array![format!(
                        "{}",
                        hmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => n as isize,
                                Index::Backward(n) => -((n + 1) as isize),
                            }
                            .to_string()
                        ))
                        .unwrap_or(&Value::Str("".into()))
                    )]);
                }
                _ => (),
            }
        } else if let Some(bmap) = self.variables.get::<types::BTreeMap>(name) {
            match selection {
                Select::All => {
                    let mut array = types::Array::new();
                    for (key, value) in bmap.iter() {
                        array.push(key.clone());
                        let f = format!("{}", value);
                        match *value {
                            Value::Str(_) => array.push(f.into()),
                            Value::Array(_) | Value::HashMap(_) | Value::BTreeMap(_) => {
                                for split in f.split_whitespace() {
                                    array.push(split.into());
                                }
                            }
                            _ => (),
                        }
                    }
                    return Some(array);
                }
                Select::Key(key) => {
                    return Some(array![format!(
                        "{}",
                        bmap.get(&*key).unwrap_or(&Value::Str("".into()))
                    )]);
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    return Some(array![format!(
                        "{}",
                        bmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => n as isize,
                                Index::Backward(n) => -((n + 1) as isize),
                            }
                            .to_string()
                        ))
                        .unwrap_or(&Value::Str("".into()))
                    )]);
                }
                _ => (),
            }
        }
        None
    }

    fn map_keys(&self, name: &str, sel: Select) -> Option<Array> {
        match self.variables.get_ref(name) {
            Some(&Value::HashMap(ref map)) => {
                Self::select(map.keys().map(|x| format!("{}", x).into()), sel, map.len())
            }
            Some(&Value::BTreeMap(ref map)) => {
                Self::select(map.keys().map(|x| format!("{}", x).into()), sel, map.len())
            }
            _ => None,
        }
    }

    fn map_values(&self, name: &str, sel: Select) -> Option<Array> {
        match self.variables.get_ref(name) {
            Some(&Value::HashMap(ref map)) => {
                Self::select(map.values().map(|x| format!("{}", x).into()), sel, map.len())
            }
            Some(&Value::BTreeMap(ref map)) => {
                Self::select(map.values().map(|x| format!("{}", x).into()), sel, map.len())
            }
            _ => None,
        }
    }

    fn tilde(&self, input: &str) -> Option<String> {
        self.variables.tilde_expansion(input, &self.directory_stack)
    }
}
