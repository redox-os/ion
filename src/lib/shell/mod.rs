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
mod job;
pub(crate) mod pipe_exec;
pub(crate) mod signals;
pub mod status;
pub mod variables;

pub use self::{
    binary::InteractiveBinary,
    fork::{Capture, Fork, IonResult},
};
pub(crate) use self::{
    flow::FlowLogic,
    job::Job,
    pipe_exec::{foreground, job_control},
};

use self::{
    directory_stack::DirectoryStack,
    escape::tilde,
    flow_control::{FlowControl, Function, FunctionError},
    foreground::ForegroundSignals,
    job_control::BackgroundProcess,
    status::*,
    variables::{GetVariable, Value, Variables},
};
use crate::{
    builtins::BuiltinMap,
    lexers::{Key, Primitive},
    parser::{assignments::value_check, pipelines::Pipeline, Expander, Select, Terminator},
    sys, types,
};
use itertools::Itertools;
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

#[derive(Debug, Error)]
pub enum IonError {
    #[error(display = "failed to fork: {}", why)]
    Fork { why: io::Error },
    #[error(display = "element does not exist")]
    DoesNotExist,
    #[error(display = "input was not terminated")]
    Unterminated,
    #[error(display = "function error: {}", why)]
    Function { why: FunctionError },
    #[error(display = "unexpected end of script: expected end block for `{}`", block)]
    UnclosedBlock { block: String },
}

#[derive(Debug, Clone, Hash)]
pub struct ShellOptions {
    /// Exit from the shell on the first error.
    pub err_exit: bool,
    /// Print commands that are to be executed.
    pub print_comms: bool,
    /// Do not execute any commands given to the shell.
    pub no_exec: bool,
    /// Hangup on exiting the shell.
    pub huponexit: bool,
    /// If set, denotes that this shell is running as a background job.
    pub is_background_shell: bool,
}

/// The shell structure is a megastructure that manages all of the state of the shell throughout
/// the entirety of the
/// program. It is initialized at the beginning of the program, and lives until the end of the
/// program.
pub struct Shell<'a> {
    /// Contains a list of built-in commands that were created when the program
    /// started.
    builtins: BuiltinMap<'a>,
    /// Contains the aliases, strings, and array variable maps.
    pub variables: Variables<'a>,
    /// Contains the current state of flow control parameters.
    flow_control: FlowControl<'a>,
    /// Contains the directory stack parameters.
    pub(crate) directory_stack: DirectoryStack,
    /// When a command is executed, the final result of that command is stored
    /// here.
    pub previous_status: i32,
    /// The job ID of the previous command sent to the background.
    pub(crate) previous_job: u32,
    /// Contains all the options relative to the shell
    opts: ShellOptions,
    /// Contains information on all of the active background processes that are being managed
    /// by the shell.
    pub(crate) background: Arc<Mutex<Vec<BackgroundProcess>>>,
    /// Used by an interactive session to know when the input is not terminated.
    pub unterminated: bool,
    /// Set when a signal is received, this will tell the flow control logic to
    /// abort.
    pub(crate) break_flow: bool,
    /// When the `fg` command is run, this will be used to communicate with the specified
    /// background process.
    foreground_signals: Arc<ForegroundSignals>,
    /// Custom callback to cleanup before exit
    prep_for_exit: Option<Box<FnMut(&mut Shell) + 'a>>,
    /// Custom callback for each command call
    on_command: Option<Box<Fn(&Shell, std::time::Duration) + 'a>>,
}

#[derive(Default)]
pub struct ShellBuilder;

impl ShellBuilder {
    pub fn as_binary<'a>(&self) -> Shell<'a> { Shell::new(false) }

    pub fn as_library<'a>(&self) -> Shell<'a> { Shell::new(true) }

    pub fn set_unique_pid(self) -> Self {
        if let Ok(pid) = sys::getpid() {
            if sys::setpgid(0, pid).is_ok() {
                let _ = sys::tcsetpgrp(0, pid);
            }
        }

        self
    }

    pub fn block_signals(self) -> Self {
        // This will block SIGTSTP, SIGTTOU, SIGTTIN, and SIGCHLD, which is required
        // for this shell to manage its own process group / children / etc.
        signals::block();

        self
    }

    pub fn install_signal_handler(self) -> Self {
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

    pub fn new() -> Self { ShellBuilder }
}

impl<'a> Shell<'a> {
    // Resets the flow control fields to their default values.
    fn reset_flow(&mut self) { self.flow_control.reset(); }

    /// A method for capturing the output of the shell, and performing actions without modifying
    /// the state of the original shell. This performs a fork, taking a closure that controls
    /// the shell in the child of the fork.
    ///
    /// The method is non-blocking, and therefore will immediately return file handles to the
    /// stdout and stderr of the child. The PID of the child is returned, which may be used to
    /// wait for and obtain the exit status.
    pub fn fork<F: FnMut(&mut Self)>(
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
        match fs::File::open(script.as_ref()) {
            Ok(script) => self.execute_script(std::io::BufReader::new(script)),
            Err(err) => eprintln!("ion: {}", err),
        }
    }

    /// A method for executing literal scripts. Given a read instance, the shell will process
    /// commands as they arrive
    pub fn execute_script<T: std::io::Read>(&mut self, lines: T) {
        if let Err(why) = self.execute_command(lines) {
            eprintln!("ion: {}", why);
            self.previous_status = FAILURE;
        }
    }

    /// A method for executing commands in the Ion shell without capturing. It takes command(s)
    /// as
    /// a string argument, parses them, and executes them the same as it would if you had
    /// executed
    /// the command(s) in the command line REPL interface for Ion. If the supplied command is
    /// not
    /// terminated, then an error will be returned.
    pub fn execute_command<T: std::io::Read>(&mut self, command: T) -> Result<i32, IonError> {
        for cmd in command
            .bytes()
            .filter_map(Result::ok)
            .batching(|bytes| Terminator::new(bytes).terminate())
        {
            self.on_command(&cmd)
        }

        if let Some(block) = self.flow_control.unclosed_block() {
            self.previous_status = FAILURE;
            Err(IonError::UnclosedBlock { block: block.to_string() })
        } else {
            Ok(self.previous_status)
        }
    }

    /// Obtains a variable, returning an empty string if it does not exist.
    pub(crate) fn get_str_or_empty(&self, name: &str) -> types::Str {
        self.variables.get_str_or_empty(name)
    }

    /// Gets any variable, if it exists within the shell's variable map.
    pub fn get<T>(&self, name: &str) -> Option<T>
    where
        Variables<'a>: GetVariable<T>,
    {
        self.variables.get::<T>(name)
    }

    /// Sets a variable of `name` with the given `value` in the shell's variable map.
    pub fn set<T: Into<Value<'a>>>(&mut self, name: &str, value: T) {
        self.variables.set(name, value);
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    pub(crate) fn run_pipeline(&mut self, mut pipeline: Pipeline<'a>) -> Option<i32> {
        let command_start_time = SystemTime::now();

        // Branch if -> input == shell command i.e. echo
        let exit_status = if let Some(main) = self.builtins.get(pipeline.items[0].command()) {
            pipeline.expand(self);
            // Run the 'main' of the command and set exit_status
            if !pipeline.requires_piping() {
                if self.opts.print_comms {
                    eprintln!("> {}", pipeline.to_string());
                }
                if self.opts.no_exec {
                    Some(SUCCESS)
                } else {
                    Some(main(&pipeline.items[0].job.args, self))
                }
            } else {
                Some(self.execute_pipeline(pipeline))
            }
        // Branch else if -> input == shell function and set the exit_status
        } else if let Some(function) =
            self.variables.get::<Function>(&pipeline.items[0].job.args[0])
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

        if let Some(ref callback) = self.on_command {
            if let Ok(elapsed_time) = command_start_time.elapsed() {
                callback(self, elapsed_time);
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

    /// Call the cleanup callback
    pub fn prep_for_exit(&mut self) {
        if let Some(mut callback) = self.prep_for_exit.take() {
            callback(self);
        }
    }

    /// Set the callback to call before exiting the shell
    #[inline]
    pub fn set_prep_for_exit(&mut self, callback: Option<Box<dyn FnMut(&mut Shell) + 'a>>) {
        self.prep_for_exit = callback;
    }

    /// Set the callback to call on each command
    #[inline]
    pub fn set_on_command(
        &mut self,
        callback: Option<Box<dyn Fn(&Shell, std::time::Duration) + 'a>>,
    ) {
        self.on_command = callback;
    }

    /// Get access to the builtins
    #[inline]
    pub fn builtins(&self) -> &BuiltinMap<'a> { &self.builtins }

    /// Get a mutable access to the builtins
    ///
    /// Warning: Previously defined functions will rely on previous versions of the builtins, even
    /// if they are redefined. It is strongly advised to avoid mutating the builtins while the shell
    /// is running
    #[inline]
    pub fn builtins_mut(&mut self) -> &mut BuiltinMap<'a> { &mut self.builtins }

    /// Access to the shell options
    #[inline]
    pub fn opts(&self) -> &ShellOptions { &self.opts }

    /// Mutable access to the shell options
    #[inline]
    pub fn opts_mut(&mut self) -> &mut ShellOptions { &mut self.opts }

    /// Cleanly exit ion
    pub fn exit(&mut self, status: i32) -> ! {
        self.prep_for_exit();
        process::exit(status);
    }

    pub fn new(is_background_shell: bool) -> Self {
        Shell {
            builtins:           BuiltinMap::default(),
            variables:          Variables::default(),
            flow_control:       FlowControl::default(),
            directory_stack:    DirectoryStack::new(),
            previous_job:       !0,
            previous_status:    0,
            opts:               ShellOptions {
                err_exit: false,
                print_comms: false,
                no_exec: false,
                huponexit: false,
                is_background_shell,
            },
            background:         Arc::new(Mutex::new(Vec::new())),
            break_flow:         false,
            foreground_signals: Arc::new(ForegroundSignals::new()),
            on_command:         None,
            prep_for_exit:      None,
            unterminated:       false,
        }
    }

    pub fn assign(&mut self, key: &Key, value: Value<'a>) -> Result<(), String> {
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

                                if let Some(var) = array.get_mut(index_num) {
                                    *var = value;
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
}

impl<'a, 'b> Expander for Shell<'b> {
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
        output.map(Into::into)
    }

    /// Expand a string variable given if its quoted / unquoted
    fn string(&self, name: &str) -> Option<types::Str> {
        if name == "?" {
            Some(types::Str::from(self.previous_status.to_string()))
        } else {
            self.get::<types::Str>(name)
        }
    }

    /// Expand an array variable with some selection
    fn array(&self, name: &str, selection: &Select) -> Option<types::Args> {
        if let Some(array) = self.variables.get::<types::Array>(name) {
            match selection {
                Select::All => {
                    return Some(types::Args::from_iter(
                        array.iter().map(|x| format!("{}", x).into()),
                    ))
                }
                Select::Index(ref id) => {
                    return id
                        .resolve(array.len())
                        .and_then(|n| array.get(n))
                        .map(|x| types::Args::from_iter(Some(format!("{}", x).into())));
                }
                Select::Range(ref range) => {
                    if let Some((start, length)) = range.bounds(array.len()) {
                        if array.len() > start {
                            return Some(
                                array
                                    .iter()
                                    .skip(start)
                                    .take(length)
                                    .map(|var| format!("{}", var).into())
                                    .collect(),
                            );
                        }
                    }
                }
                _ => (),
            }
        } else if let Some(hmap) = self.variables.get::<types::HashMap>(name) {
            match selection {
                Select::All => {
                    let mut array = types::Args::new();
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
                    return Some(args![format!(
                        "{}",
                        hmap.get(&*key).unwrap_or(&Value::Str("".into()))
                    )]);
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    return Some(args![format!(
                        "{}",
                        hmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => *n as isize,
                                Index::Backward(n) => -((*n + 1) as isize),
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
                    let mut array = types::Args::new();
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
                    return Some(args![format!(
                        "{}",
                        bmap.get(&*key).unwrap_or(&Value::Str("".into()))
                    )]);
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    return Some(args![format!(
                        "{}",
                        bmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => *n as isize,
                                Index::Backward(n) => -((*n + 1) as isize),
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

    fn map_keys(&self, name: &str, sel: &Select) -> Option<types::Args> {
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

    fn map_values(&self, name: &str, sel: &Select) -> Option<types::Args> {
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
        tilde(
            input,
            &self.directory_stack,
            self.variables.get::<types::Str>("OLDPWD").as_ref().map(types::Str::as_str),
        )
    }
}
