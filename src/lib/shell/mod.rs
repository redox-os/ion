mod assignments;
mod colors;
mod directory_stack;
mod flow;
pub(crate) mod flow_control;
mod fork;
mod fork_function;
mod job;
mod pipe_exec;
mod shell_expand;
pub(crate) mod signals;
pub mod status;
pub mod variables;

pub(crate) use self::job::Job;
use self::{
    directory_stack::DirectoryStack,
    flow_control::{Block, FunctionError, Statement},
    foreground::ForegroundSignals,
    fork::{Fork, IonResult},
    pipe_exec::foreground,
    status::*,
    variables::Variables,
};
pub use self::{fork::Capture, pipe_exec::job_control::BackgroundProcess, variables::Value};
use crate::{
    builtins::BuiltinMap,
    lexers::{Key, Primitive},
    parser::{assignments::value_check, pipelines::Pipeline, Terminator},
    sys, types,
};
use itertools::Itertools;
use std::{
    borrow::Cow,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
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
    variables: Variables<'a>,
    /// Contains the current state of flow control parameters.
    flow_control: Block<'a>,
    /// Contains the directory stack parameters.
    directory_stack: DirectoryStack,
    /// When a command is executed, the final result of that command is stored
    /// here.
    previous_status: i32,
    /// The job ID of the previous command sent to the background.
    previous_job: u32,
    /// Contains all the options relative to the shell
    opts: ShellOptions,
    /// Contains information on all of the active background processes that are being managed
    /// by the shell.
    pub(crate) background: Arc<Mutex<Vec<BackgroundProcess>>>,
    /// Used by an interactive session to know when the input is not terminated.
    pub unterminated: bool,
    /// Set when a signal is received, this will tell the flow control logic to
    /// abort.
    break_flow: bool,
    /// When the `fg` command is run, this will be used to communicate with the specified
    /// background process.
    foreground_signals: Arc<ForegroundSignals>,
    /// Custom callback to cleanup before exit
    prep_for_exit: Option<Box<FnMut(&mut Shell) + 'a>>,
    /// Custom callback for each command call
    on_command: Option<Box<Fn(&Shell, std::time::Duration) + 'a>>,
}

pub struct EmptyBlockError;

impl fmt::Display for EmptyBlockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not exit the current block since it does not exist!")
    }
}

// Implement std::fmt::Debug for AppError
impl fmt::Debug for EmptyBlockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "EmptyBlockError {{ file: {}, line: {} }}", file!(), line!())
    }
}

impl<'a> Shell<'a> {
    const CONFIG_FILE_NAME: &'static str = "initrc";

    /// Set the shell as the terminal primary executable
    pub fn set_unique_pid(&self) {
        if let Ok(pid) = sys::getpid() {
            if sys::setpgid(0, pid).is_ok() {
                let _ = sys::tcsetpgrp(0, pid);
            }
        }
    }

    /// Install signal handlers necessary for the shell to work
    fn install_signal_handler() {
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
    }

    pub fn binary() -> Self { Self::new(false) }

    pub fn library() -> Self { Self::new(true) }

    pub fn new(is_background_shell: bool) -> Self {
        Self::with_builtins(BuiltinMap::default().with_shell_dangerous(), is_background_shell)
    }

    /// Create a shell with custom builtins
    pub fn with_builtins(builtins: BuiltinMap<'a>, is_background_shell: bool) -> Self {
        Self::install_signal_handler();

        // This will block SIGTSTP, SIGTTOU, SIGTTIN, and SIGCHLD, which is required
        // for this shell to manage its own process group / children / etc.
        signals::block();

        Shell {
            builtins,
            variables: Variables::default(),
            flow_control: Block::with_capacity(5),
            directory_stack: DirectoryStack::new(),
            previous_job: !0,
            previous_status: SUCCESS,
            opts: ShellOptions {
                err_exit: false,
                print_comms: false,
                no_exec: false,
                huponexit: false,
                is_background_shell,
            },
            background: Arc::new(Mutex::new(Vec::new())),
            break_flow: false,
            foreground_signals: Arc::new(ForegroundSignals::new()),
            on_command: None,
            prep_for_exit: None,
            unterminated: false,
        }
    }

    pub fn rotate_right(&mut self, num: usize) -> Result<(), Cow<'static, str>> {
        self.directory_stack.rotate_right(num)
    }

    pub fn rotate_left(&mut self, num: usize) -> Result<(), Cow<'static, str>> {
        self.directory_stack.rotate_left(num)
    }

    pub fn swap(&mut self, index: usize) -> Result<(), Cow<'static, str>> {
        self.directory_stack.swap(index)
    }

    pub fn set_current_dir_by_index(&self, index: usize) -> Result<(), Cow<'static, str>> {
        self.directory_stack.set_current_dir_by_index(index)
    }

    pub fn cd<T: AsRef<str>>(&mut self, dir: Option<T>) -> Result<(), Cow<'static, str>> {
        self.directory_stack.cd(dir, &mut self.variables)
    }

    pub fn pushd(&mut self, path: PathBuf, keep_front: bool) -> Result<(), Cow<'static, str>> {
        self.directory_stack.pushd(path, keep_front, &mut self.variables)
    }

    pub fn popd(&mut self, index: usize) -> Option<PathBuf> { self.directory_stack.popd(index) }

    pub fn dir_stack(&self) -> impl DoubleEndedIterator<Item = &PathBuf> + ExactSizeIterator {
        self.directory_stack.dirs()
    }

    pub fn clear_dir_stack(&mut self) { self.directory_stack.clear() }

    /// Resets the flow control fields to their default values.
    pub fn reset_flow(&mut self) { self.flow_control.clear(); }

    /// Exit the current block
    pub fn exit_block(&mut self) -> Result<(), EmptyBlockError> {
        self.flow_control.pop().map(|_| ()).ok_or(EmptyBlockError)
    }

    /// Get the depth of the current block
    pub fn block_len(&self) -> usize { self.flow_control.len() }

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
        if let Some(Value::Function(function)) = self.variables.get_ref(name).cloned() {
            function
                .execute(self, args)
                .map(|_| self.previous_status)
                .map_err(|err| IonError::Function { why: err })
        } else {
            Err(IonError::DoesNotExist)
        }
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

        if let Some(block) = self.flow_control.last().map(Statement::short) {
            self.previous_status = FAILURE;
            Err(IonError::UnclosedBlock { block: block.to_string() })
        } else {
            Ok(self.previous_status)
        }
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    pub fn run_pipeline(&mut self, mut pipeline: Pipeline<'a>) -> Option<i32> {
        let command_start_time = SystemTime::now();

        pipeline.expand(self);
        // Branch if -> input == shell command i.e. echo
        let exit_status = if let Some(main) = self.builtins.get(pipeline.items[0].command()) {
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
        } else if let Some(Value::Function(function)) =
            self.variables.get_ref(&pipeline.items[0].job.args[0]).cloned()
        {
            if !pipeline.requires_piping() {
                match function.execute(self, &pipeline.items[0].job.args) {
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
            Some(self.execute_pipeline(pipeline))
        };

        if let Some(ref callback) = self.on_command {
            if let Ok(elapsed_time) = command_start_time.elapsed() {
                callback(self, elapsed_time);
            }
        }

        // Retrieve the exit_status and set the $? variable and history.previous_status
        if let Some(code) = exit_status {
            self.variables_mut().set("?", code.to_string());
            self.previous_status = code;
        }

        exit_status
    }

    pub fn previous_job(&self) -> Option<u32> {
        if self.previous_job == !0 {
            None
        } else {
            Some(self.previous_job)
        }
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
        match base_dirs.find_config_file(Self::CONFIG_FILE_NAME) {
            Some(initrc) => self.execute_file(&initrc),
            None => {
                if let Err(err) = Self::create_config_file(base_dirs, Self::CONFIG_FILE_NAME) {
                    eprintln!("ion: could not create config file: {}", err);
                }
            }
        }
    }

    fn create_config_file(base_dirs: BaseDirectories, file_name: &str) -> Result<(), io::Error> {
        let path = base_dirs.place_config_file(file_name)?;
        OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(())
    }

    /// Call the cleanup callback
    pub fn prep_for_exit(&mut self) {
        if let Some(mut callback) = self.prep_for_exit.take() {
            callback(self);
        }
    }

    /// Set the callback to call before exiting the shell
    pub fn set_prep_for_exit(&mut self, callback: Option<Box<dyn FnMut(&mut Shell) + 'a>>) {
        self.prep_for_exit = callback;
    }

    /// Set the callback to call on each command
    pub fn set_on_command(
        &mut self,
        callback: Option<Box<dyn Fn(&Shell, std::time::Duration) + 'a>>,
    ) {
        self.on_command = callback;
    }

    /// Get access to the builtins
    pub fn builtins(&self) -> &BuiltinMap<'a> { &self.builtins }

    /// Get a mutable access to the builtins
    ///
    /// Warning: Previously defined functions will rely on previous versions of the builtins, even
    /// if they are redefined. It is strongly advised to avoid mutating the builtins while the shell
    /// is running
    pub fn builtins_mut(&mut self) -> &mut BuiltinMap<'a> { &mut self.builtins }

    /// Access to the shell options
    pub fn opts(&self) -> &ShellOptions { &self.opts }

    /// Mutable access to the shell options
    pub fn opts_mut(&mut self) -> &mut ShellOptions { &mut self.opts }

    /// Access to the variables
    pub fn variables(&self) -> &Variables<'a> { &self.variables }

    /// Mutable access to the variables
    pub fn variables_mut(&mut self) -> &mut Variables<'a> { &mut self.variables }

    /// Get the last command's return code and/or the code for the error
    pub fn previous_status(&self) -> i32 { self.previous_status }

    /// Cleanly exit ion
    pub fn exit(&mut self, status: Option<i32>) -> ! {
        self.prep_for_exit();
        process::exit(status.unwrap_or(self.previous_status));
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
