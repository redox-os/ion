mod assignments;
mod colors;
mod directory_stack;
mod flow;
/// The various blocks
pub mod flow_control;
mod fork;
mod fork_function;
mod job;
mod pipe_exec;
mod shell_expand;
mod signals;
pub(crate) mod sys;
/// Variables for the shell
pub mod variables;

pub(crate) use self::job::Job;
use self::{
    directory_stack::DirectoryStack,
    flow_control::{Block, Function, FunctionError, Statement},
    fork::{Fork, IonResult},
    pipe_exec::foreground,
    variables::Variables,
};
pub use self::{
    flow::BlockError,
    fork::Capture,
    pipe_exec::{job_control::BackgroundProcess, PipelineError},
    variables::Value,
};
use crate::{
    assignments::value_check,
    builtins::{BuiltinMap, Status},
    expansion::{pipelines::Pipeline, Error as ExpansionError},
    parser::{
        lexers::{Key, Primitive},
        Error as ParseError, Terminator,
    },
};
use err_derive::Error;
use itertools::Itertools;
use nix::sys::signal::{self, SigHandler};
use std::{
    io::{self, Write},
    ops::{Deref, DerefMut},
    sync::{atomic::Ordering, Arc, Mutex},
    time::SystemTime,
};

/// Errors from execution
#[derive(Debug, Error)]
pub enum IonError {
    // Parse-time error
    /// Parsing failed
    #[error(display = "syntax error: {}", _0)]
    InvalidSyntax(#[error(cause)] ParseError),
    /// Incorrect order of blocks
    #[error(display = "block error: {}", _0)]
    StatementFlowError(#[error(cause)] BlockError),

    // Run time errors
    /// Function execution error
    #[error(display = "function error: {}", _0)]
    Function(#[error(cause)] FunctionError),
    /// Failed to run a pipeline
    #[error(display = "pipeline execution error: {}", _0)]
    PipelineExecutionError(#[error(cause)] PipelineError),
    /// Could not properly expand to a pipeline
    #[error(display = "expansion error: {}", _0)]
    ExpansionError(#[error(cause)] ExpansionError<IonError>),
}

impl From<ParseError> for IonError {
    fn from(cause: ParseError) -> Self { IonError::InvalidSyntax(cause) }
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

impl From<ExpansionError<IonError>> for IonError {
    fn from(cause: ExpansionError<Self>) -> Self { IonError::ExpansionError(cause) }
}

/// Options for the shell
#[derive(Debug, Clone, Hash)]
pub struct Options {
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
    opts: Options,
    /// Contains information on all of the active background processes that are being managed
    /// by the shell.
    background: Arc<Mutex<Vec<BackgroundProcess>>>,
    /// Used by an interactive session to know when the input is not terminated.
    pub unterminated: bool,
    /// When the `fg` command is run, this will be used to communicate with the specified
    /// background process.
    foreground_signals: Arc<foreground::Signals>,
    /// Custom callback for each command call
    on_command: Option<Box<dyn Fn(&Shell<'_>, std::time::Duration) + 'a>>,
}

impl<'a> Default for Shell<'a> {
    fn default() -> Self { Self::new() }
}

impl<'a> Shell<'a> {
    /// Install signal handlers necessary for the shell to work
    fn install_signal_handler() {
        extern "C" fn handler(signal: i32) {
            let signal = signal::Signal::from_c_int(signal).unwrap();
            let signal = match signal {
                signal::Signal::SIGINT => signals::SIGINT,
                signal::Signal::SIGHUP => signals::SIGHUP,
                signal::Signal::SIGTERM => signals::SIGTERM,
                _ => unreachable!(),
            };

            signals::PENDING.store(signal as usize, Ordering::SeqCst);
        }

        extern "C" fn sigpipe_handler(signal: i32) {
            let _ = io::stdout().flush();
            let _ = io::stderr().flush();
            unsafe { nix::libc::_exit(127 + signal) };
        }

        unsafe {
            let _ = signal::signal(signal::Signal::SIGHUP, SigHandler::Handler(handler));
            let _ = signal::signal(signal::Signal::SIGINT, SigHandler::Handler(handler));
            let _ = signal::signal(signal::Signal::SIGTERM, SigHandler::Handler(handler));
            let _ = signal::signal(signal::Signal::SIGPIPE, SigHandler::Handler(sigpipe_handler));
        }
    }

    /// Create a new shell with default settings
    pub fn new() -> Self { Self::with_builtins(BuiltinMap::default()) }

    /// Create a shell with custom builtins
    pub fn with_builtins(builtins: BuiltinMap<'a>) -> Self {
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
            opts: Options {
                err_exit:            false,
                print_comms:         false,
                no_exec:             false,
                huponexit:           false,
                is_background_shell: true,
            },
            background: Arc::new(Mutex::new(Vec::new())),
            foreground_signals: Arc::new(foreground::Signals::new()),
            on_command: None,
            unterminated: false,
        }
    }

    /// Access the directory stack
    pub const fn dir_stack(&self) -> &DirectoryStack { &self.directory_stack }

    /// Mutable access to the directory stack
    pub fn dir_stack_mut(&mut self) -> &mut DirectoryStack { &mut self.directory_stack }

    /// Resets the flow control fields to their default values.
    pub fn reset_flow(&mut self) { self.flow_control.clear(); }

    /// Exit the current block
    pub fn exit_block(&mut self) -> Result<(), BlockError> {
        self.flow_control.pop().map(|_| ()).ok_or(BlockError::UnmatchedEnd)
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
    fn fork<F: FnMut(&mut Self) -> Result<(), IonError>>(
        &self,
        capture: Capture,
        child_func: F,
    ) -> nix::Result<IonResult> {
        Fork::new(self, capture).exec(child_func)
    }

    /// A method for executing a function, using `args` as the input.
    pub fn execute_function<S: AsRef<str>>(
        &mut self,
        function: &Function<'a>,
        args: &[S],
    ) -> Result<Status, IonError> {
        function.clone().execute(self, args)?;
        Ok(self.previous_status)
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

        if let Some(block) = self.flow_control.last().map(Statement::to_string) {
            self.previous_status = Status::from_exit_code(1);
            Err(IonError::StatementFlowError(BlockError::UnclosedBlock(block)))
        } else {
            Ok(self.previous_status)
        }
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    pub fn run_pipeline(&mut self, pipeline: &Pipeline<'a>) -> Result<Status, IonError> {
        let command_start_time = SystemTime::now();

        let pipeline = pipeline.expand(self)?;
        // A string representing the command is stored here.
        if self.opts.print_comms {
            eprintln!("> {}", pipeline);
        }

        // Don't execute commands when the `-n` flag is passed.
        let exit_status = if self.opts.no_exec {
            Ok(Status::SUCCESS)
        // Branch else if -> input == shell command i.e. echo
        } else if let Some(main) = self.builtins.get(pipeline.items[0].command()) {
            // Run the 'main' of the command and set exit_status
            if pipeline.requires_piping() {
                self.execute_pipeline(pipeline).map_err(Into::into)
            } else {
                Ok(main(&pipeline.items[0].job.args, self))
            }
        // Branch else if -> input == shell function and set the exit_status
        } else if let Some(Value::Function(function)) =
            self.variables.get(&pipeline.items[0].job.args[0]).cloned()
        {
            if pipeline.requires_piping() {
                self.execute_pipeline(pipeline).map_err(Into::into)
            } else {
                function.execute(self, &pipeline.items[0].job.args).map(|_| self.previous_status)
            }
        } else {
            self.execute_pipeline(pipeline).map_err(Into::into)
        }?;

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

    /// Get the pid of the last executed job
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
    pub const fn builtins(&self) -> &BuiltinMap<'a> { &self.builtins }

    /// Get a mutable access to the builtins
    ///
    /// Warning: Previously defined functions will rely on previous versions of the builtins, even
    /// if they are redefined. It is strongly advised to avoid mutating the builtins while the shell
    /// is running
    pub fn builtins_mut(&mut self) -> &mut BuiltinMap<'a> { &mut self.builtins }

    /// Access to the shell options
    pub const fn opts(&self) -> &Options { &self.opts }

    /// Mutable access to the shell options
    pub fn opts_mut(&mut self) -> &mut Options { &mut self.opts }

    /// Access to the variables
    pub const fn variables(&self) -> &Variables<'a> { &self.variables }

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

    /// Get the last command's return code and/or the code for the error
    pub const fn previous_status(&self) -> Status { self.previous_status }

    fn assign(&mut self, key: &Key<'_>, value: Value<Function<'a>>) -> Result<(), String> {
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
