mod assignments;
mod colors;
mod directory_stack;
mod flow;
pub mod flow_control;
mod fork;
mod fork_function;
mod job;
mod pipe_exec;
mod shell_expand;
mod signals;
pub mod variables;

pub(crate) use self::job::Job;
use self::{
    directory_stack::DirectoryStack,
    flow_control::{Block, BlockError, FunctionError, Statement},
    foreground::ForegroundSignals,
    fork::{Fork, IonResult},
    pipe_exec::foreground,
    variables::Variables,
};
pub use self::{
    fork::Capture,
    pipe_exec::{job_control::BackgroundProcess, PipelineError},
    variables::Value,
};
use crate::{
    builtins::{BuiltinMap, Status},
    lexers::{Key, Primitive},
    parser::{
        assignments::value_check, pipelines::Pipeline, shell_expand::ExpansionError, ParseError,
        StatementError, Terminator,
    },
    sys, types,
};
use err_derive::Error;
use itertools::Itertools;
use std::{
    fs,
    io::{self, Write},
    ops::{Deref, DerefMut},
    path::Path,
    sync::{atomic::Ordering, Arc, Mutex},
    time::SystemTime,
};

#[derive(Debug, Error)]
pub enum IonError {
    #[error(display = "failed to fork: {}", _0)]
    Fork(#[error(cause)] io::Error),
    #[error(display = "element does not exist")]
    DoesNotExist,
    #[error(display = "input was not terminated")]
    Unterminated,
    #[error(display = "function error: {}", _0)]
    Function(#[error(cause)] FunctionError),
    #[error(display = "unexpected end of script: expected end block for `{}`", _0)]
    UnclosedBlock(String),
    #[error(display = "syntax error: {}", _0)]
    InvalidSyntax(#[error(cause)] ParseError),
    #[error(display = "block error: {}", _0)]
    StatementFlowError(#[error(cause)] BlockError),
    #[error(display = "statement error: {}", _0)]
    UnterminatedStatementError(#[error(cause)] StatementError),
    #[error(display = "could not exit the current block since it does not exist!")]
    EmptyBlock,
    #[error(display = "could not execute file '{}': {}", _0, _1)]
    FileExecutionError(String, #[error(cause)] io::Error),
    #[error(display = "pipeline execution error: {}", _0)]
    PipelineExecutionError(#[error(cause)] PipelineError),
    #[error(display = "expansion error: {}", _0)]
    ExpansionError(#[error(cause)] ExpansionError),
}

impl From<ParseError> for IonError {
    fn from(cause: ParseError) -> Self { IonError::InvalidSyntax(cause) }
}

impl From<StatementError> for IonError {
    fn from(cause: StatementError) -> Self { IonError::UnterminatedStatementError(cause) }
}

impl From<FunctionError> for IonError {
    fn from(cause: FunctionError) -> Self { IonError::Function(cause) }
}

impl From<BlockError> for IonError {
    fn from(cause: BlockError) -> Self { IonError::StatementFlowError(cause) }
}

impl From<PipelineError> for IonError {
    fn from(cause: PipelineError) -> Self { IonError::PipelineExecutionError(cause) }
}

impl From<io::Error> for IonError {
    fn from(cause: io::Error) -> Self { IonError::Fork(cause) }
}

impl From<ExpansionError> for IonError {
    fn from(cause: ExpansionError) -> Self { IonError::ExpansionError(cause) }
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
    previous_status: Status,
    /// The job ID of the previous command sent to the background.
    previous_job: usize,
    /// Contains all the options relative to the shell
    opts: ShellOptions,
    /// Contains information on all of the active background processes that are being managed
    /// by the shell.
    background: Arc<Mutex<Vec<BackgroundProcess>>>,
    /// Used by an interactive session to know when the input is not terminated.
    pub unterminated: bool,
    /// When the `fg` command is run, this will be used to communicate with the specified
    /// background process.
    foreground_signals: Arc<ForegroundSignals>,
    /// Custom callback for each command call
    on_command: Option<Box<dyn Fn(&Shell<'_>, std::time::Duration) + 'a>>,
}

impl<'a> Shell<'a> {
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
        Self::with_builtins(BuiltinMap::default(), is_background_shell)
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
            previous_status: Status::SUCCESS,
            opts: ShellOptions {
                err_exit: false,
                print_comms: false,
                no_exec: false,
                huponexit: false,
                is_background_shell,
            },
            background: Arc::new(Mutex::new(Vec::new())),
            foreground_signals: Arc::new(ForegroundSignals::new()),
            on_command: None,
            unterminated: false,
        }
    }

    pub fn dir_stack(&self) -> &DirectoryStack { &self.directory_stack }

    pub fn dir_stack_mut(&mut self) -> &mut DirectoryStack { &mut self.directory_stack }

    /// Resets the flow control fields to their default values.
    pub fn reset_flow(&mut self) { self.flow_control.clear(); }

    /// Exit the current block
    pub fn exit_block(&mut self) -> Result<(), IonError> {
        self.flow_control.pop().map(|_| ()).ok_or(IonError::EmptyBlock)
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
    pub fn fork<F: FnMut(&mut Self) -> Result<(), IonError>>(
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
    ) -> Result<Status, IonError> {
        if let Some(Value::Function(function)) = self.variables.get_ref(name).cloned() {
            function.execute(self, args)?;
            Ok(self.previous_status)
        } else {
            Err(IonError::DoesNotExist)
        }
    }

    /// A method for executing scripts in the Ion shell without capturing. Given a `Path`, this
    /// method will attempt to execute that file as a script, and then returns the final exit
    /// status of the evaluated script.
    pub fn execute_file<P: AsRef<Path>>(&mut self, script: P) -> Result<Status, IonError> {
        match fs::File::open(script.as_ref()) {
            Ok(script) => self.execute_command(std::io::BufReader::new(script)),
            Err(cause) => {
                Err(IonError::FileExecutionError(script.as_ref().to_string_lossy().into(), cause))
            }
        }
    }

    /// A method for executing commands in the Ion shell without capturing. It takes command(s)
    /// as
    /// a string argument, parses them, and executes them the same as it would if you had
    /// executed
    /// the command(s) in the command line REPL interface for Ion. If the supplied command is
    /// not
    /// terminated, then an error will be returned.
    pub fn execute_command<T: std::io::Read>(&mut self, command: T) -> Result<Status, IonError> {
        for cmd in command
            .bytes()
            .filter_map(Result::ok)
            .batching(|bytes| Terminator::new(bytes).terminate())
        {
            self.on_command(&cmd)?;
        }

        if let Some(block) = self.flow_control.last().map(Statement::short) {
            self.previous_status = Status::from_exit_code(1);
            Err(IonError::UnclosedBlock(block.into()))
        } else {
            Ok(self.previous_status)
        }
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    pub fn run_pipeline(&mut self, pipeline: Pipeline<'a>) -> Result<Status, IonError> {
        let command_start_time = SystemTime::now();

        let pipeline = pipeline.expand(self)?;
        // A string representing the command is stored here.
        if self.opts.print_comms {
            eprintln!("> {}", pipeline);
        }

        // Don't execute commands when the `-n` flag is passed.
        let exit_status = if self.opts.no_exec {
            Status::SUCCESS
        // Branch else if -> input == shell command i.e. echo
        } else if let Some(main) = self.builtins.get(pipeline.items[0].command()) {
            // Run the 'main' of the command and set exit_status
            if !pipeline.requires_piping() {
                main(&pipeline.items[0].job.args, self)
            } else {
                self.execute_pipeline(pipeline)?
            }
        // Branch else if -> input == shell function and set the exit_status
        } else if let Some(Value::Function(function)) =
            self.variables.get_ref(&pipeline.items[0].job.args[0]).cloned()
        {
            if !pipeline.requires_piping() {
                function.execute(self, &pipeline.items[0].job.args).map(|_| self.previous_status)?
            } else {
                self.execute_pipeline(pipeline)?
            }
        } else {
            self.execute_pipeline(pipeline)?
        };

        if let Some(ref callback) = self.on_command {
            if let Ok(elapsed_time) = command_start_time.elapsed() {
                callback(self, elapsed_time);
            }
        }

        if self.opts.err_exit && !exit_status.is_success() {
            Err(PipelineError::EarlyExit)?
        }

        Ok(exit_status)
    }

    pub fn previous_job(&self) -> Option<usize> {
        if self.previous_job == !0 {
            None
        } else {
            Some(self.previous_job)
        }
    }

    /// Set the callback to call on each command
    pub fn set_on_command(
        &mut self,
        callback: Option<Box<dyn Fn(&Shell<'_>, std::time::Duration) + 'a>>,
    ) {
        self.on_command = callback;
    }

    /// Set the callback to call on each command
    pub fn on_command_mut(
        &mut self,
    ) -> &mut Option<Box<dyn Fn(&Shell<'_>, std::time::Duration) + 'a>> {
        &mut self.on_command
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

    /// Access to the variables
    pub fn background_jobs<'mutex>(
        &'mutex self,
    ) -> impl Deref<Target = Vec<BackgroundProcess>> + 'mutex {
        self.background.lock().expect("Could not lock the mutex")
    }

    /// Mutable access to the variables
    pub fn background_jobs_mut<'mutex>(
        &'mutex mut self,
    ) -> impl DerefMut<Target = Vec<BackgroundProcess>> + 'mutex {
        self.background.lock().expect("Could not lock the mutex")
    }

    pub fn suspend(&self) { signals::suspend(0); }

    /// Get the last command's return code and/or the code for the error
    pub fn previous_status(&self) -> Status { self.previous_status }

    pub fn assign(&mut self, key: &Key<'_>, value: Value<'a>) -> Result<(), String> {
        match (&key.kind, &value) {
            (Primitive::Indexed(ref index_name, ref index_kind), Value::Str(_)) => {
                let index = value_check(self, index_name, index_kind)
                    .map_err(|why| format!("{}: {}", key.name, why))?;

                match index {
                    Value::Str(index) => {
                        let lhs = self
                            .variables
                            .get_mut(key.name)
                            .ok_or_else(|| "index value does not exist".to_string())?;

                        match lhs {
                            Value::HashMap(hmap) => {
                                let _ = hmap.insert(index, value);
                                Ok(())
                            }
                            Value::BTreeMap(bmap) => {
                                let _ = bmap.insert(index, value);
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
