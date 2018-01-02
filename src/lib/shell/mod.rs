mod assignments;
mod binary;
mod completer;
mod flow;
mod fork;
mod history;
mod job;
mod pipe_exec;
pub(crate) mod colors;
pub(crate) mod directory_stack;
pub mod flags;
pub(crate) mod plugins;
pub(crate) mod flow_control;
pub(crate) mod signals;
pub mod status;
pub mod variables;

pub use self::binary::Binary;
pub(crate) use self::flow::FlowLogic;
pub use self::fork::{Capture, Fork, IonResult};
pub(crate) use self::history::{IgnoreSetting, ShellHistory};
pub(crate) use self::job::{Job, JobKind};
pub(crate) use self::pipe_exec::{foreground, job_control};

use self::directory_stack::DirectoryStack;
use self::flags::*;
use self::flow_control::{FlowControl, Function, FunctionError};
use self::foreground::ForegroundSignals;
use self::job_control::{BackgroundProcess, JobControl};
use self::pipe_exec::PipelineExecution;
use self::status::*;
use self::variables::Variables;
use builtins::{BuiltinMap, BUILTINS};
use fnv::FnvHashMap;
use liner::Context;
use parser::{ArgumentSplitter, Expander, Select};
use parser::Terminator;
use parser::pipelines::Pipeline;
use smallvec::SmallVec;
use std::fs::File;
use std::io::{self, Read};
use std::iter::FromIterator;
use std::ops::Deref;
use std::path::Path;
use std::process;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use sys;
use types::*;
use xdg::BaseDirectories;

#[derive(Debug, Fail)]
pub enum IonError {
    #[fail(display = "failed to fork: {}", why)] Fork { why: io::Error },
    #[fail(display = "element does not exist")] DoesNotExist,
    #[fail(display = "input was not terminated")] Unterminated,
    #[fail(display = "function error: {}", why)] Function { why: FunctionError },
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
    pub(crate) variables: Variables,
    /// Contains the current state of flow control parameters.
    flow_control: FlowControl,
    /// Contains the directory stack parameters.
    pub(crate) directory_stack: DirectoryStack,
    /// Contains all of the user-defined functions that have been created.
    pub(crate) functions: FnvHashMap<Identifier, Function>,
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

pub struct ShellBuilder;

impl ShellBuilder {
    pub fn new() -> ShellBuilder { ShellBuilder }

    pub fn install_signal_handler(self) -> ShellBuilder {
        extern "C" fn handler(signal: i32) {
            let signal = match signal {
                sys::SIGINT => signals::SIGINT,
                sys::SIGHUP => signals::SIGHUP,
                sys::SIGTERM => signals::SIGTERM,
                _ => unreachable!(),
            };

            signals::PENDING.store(signal, Ordering::SeqCst);
        }

        let _ = sys::signal(sys::SIGHUP, handler);
        let _ = sys::signal(sys::SIGINT, handler);
        let _ = sys::signal(sys::SIGTERM, handler);

        extern "C" fn sigpipe_handler(signal: i32) { sys::fork_exit(127 + signal); }

        let _ = sys::signal(sys::SIGPIPE, sigpipe_handler);

        self
    }

    pub fn block_signals(self) -> ShellBuilder {
        // This will block SIGTSTP, SIGTTOU, SIGTTIN, and SIGCHLD, which is required
        // for this shell to manage its own process group / children / etc.
        signals::block();

        self
    }

    pub fn set_unique_pid(self) -> ShellBuilder {
        if let Ok(pid) = sys::getpid() {
            if sys::setpgid(0, pid).is_ok() {
                let _ = sys::tcsetpgrp(0, pid);
            }
        }

        self
    }

    pub fn as_library(self) -> Shell { Shell::new(true) }

    pub fn as_binary(self) -> Shell { Shell::new(false) }
}

impl<'a> Shell {
    pub(crate) fn new(is_library: bool) -> Shell {
        Shell {
            builtins: BUILTINS,
            context: None,
            variables: Variables::default(),
            flow_control: FlowControl::default(),
            directory_stack: DirectoryStack::new(),
            functions: FnvHashMap::default(),
            previous_job: !0,
            previous_status: 0,
            flags: 0,
            background: Arc::new(Mutex::new(Vec::new())),
            is_background_shell: false,
            is_library,
            break_flow: false,
            foreground_signals: Arc::new(ForegroundSignals::new()),
            ignore_setting: IgnoreSetting::default(),
        }
    }

    pub(crate) fn next_signal(&self) -> Option<i32> {
        match signals::PENDING.swap(0, Ordering::SeqCst) {
            0 => None,
            signals::SIGINT => Some(sys::SIGINT),
            signals::SIGHUP => Some(sys::SIGHUP),
            signals::SIGTERM => Some(sys::SIGTERM),
            _ => unreachable!(),
        }
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
            let context = self.context.as_mut().unwrap();
            context.history.commit_history();
        }
    }

    pub(crate) fn exit(&mut self, status: i32) -> ! {
        self.prep_for_exit();
        process::exit(status);
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
            Some(initrc) => {
                if let Err(err) = self.execute_script(&initrc) {
                    eprintln!("ion: {}", err);
                }
            }
            None => {
                if let Err(err) = base_dirs.place_config_file("initrc") {
                    eprintln!("ion: could not create initrc file: {}", err);
                }
            }
        }
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    pub(crate) fn run_pipeline(&mut self, pipeline: &mut Pipeline) -> Option<i32> {
        let command_start_time = SystemTime::now();

        // Expand any aliases found
        for job_no in 0..pipeline.items.len() {
            let mut last_command = String::with_capacity(32);
            loop {
                let possible_alias = {
                    let key: &str = pipeline.items[job_no].job.command.as_ref();
                    if &last_command == key {
                        break;
                    }
                    last_command.clear();
                    last_command.push_str(key);
                    self.variables.aliases.get(key)
                };

                if let Some(alias) = possible_alias {
                    let new_args = ArgumentSplitter::new(alias)
                        .map(String::from)
                        .chain(pipeline.items[job_no].job.args.drain().skip(1))
                        .collect::<Array>();
                    if let Some(builtin) = BUILTINS.get(&new_args[0]) {
                        pipeline.items[job_no].job.builtin = Some(builtin.main);
                    } else {
                        pipeline.items[job_no].job.command = new_args[0].clone().into();
                    }
                    pipeline.items[job_no].job.args = new_args;
                }
            }
        }

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
                    let small: SmallVec<[&str; 4]> = borrowed.iter().map(|x| x as &str).collect();
                    Some(main(&small, self))
                }
            } else {
                Some(self.execute_pipeline(pipeline))
            }
        // Branch else if -> input == shell function and set the exit_status
        } else if let Some(function) = self.functions.get(&pipeline.items[0].job.command).cloned() {
            if !pipeline.requires_piping() {
                let args: &[String] = pipeline.items[0].job.args.deref();
                let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
                match function.execute(self, &args) {
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
            if "1" == self.variables.get_var_or_empty("RECORD_SUMMARY") {
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
            self.set_var("?", &code.to_string());
            self.previous_status = code;
        }

        exit_status
    }

    /// Sets a variable of `name` with the given `value` in the shell's
    /// variable map.
    pub fn set_var(&mut self, name: &str, value: &str) { self.variables.set_var(name, value); }

    /// Gets a string variable, if it exists within the shell's variable map.
    pub fn get_var(&self, name: &str) -> Option<String> { self.variables.get_var(name) }

    /// Obtains a variable, returning an empty string if it does not exist.
    pub(crate) fn get_var_or_empty(&self, name: &str) -> String {
        self.variables.get_var_or_empty(name)
    }

    /// Gets an array variable, if it exists within the shell's array map.
    pub fn get_array(&self, name: &str) -> Option<&[String]> {
        self.variables.get_array(name).map(SmallVec::as_ref)
    }

    /// A method for executing commands in the Ion shell without capturing. It takes command(s)
    /// as
    /// a string argument, parses them, and executes them the same as it would if you had
    /// executed
    /// the command(s) in the command line REPL interface for Ion. If the supplied command is
    /// not
    /// terminated, then an error will be returned.
    pub fn execute_command<CMD>(&mut self, command: CMD) -> Result<i32, IonError>
    where
        CMD: Into<Terminator>,
    {
        let mut terminator = command.into();
        if terminator.is_terminated() {
            self.on_command(&terminator.consume());
            Ok(self.previous_status)
        } else {
            Err(IonError::Unterminated)
        }
    }

    /// A method for executing scripts in the Ion shell without capturing. Given a `Path`, this
    /// method will attempt to execute that file as a script, and then returns the final exit
    /// status of the evaluated script.
    pub fn execute_script<SCRIPT: AsRef<Path>>(&mut self, script: SCRIPT) -> io::Result<i32> {
        let mut script = File::open(script.as_ref())?;
        let capacity = script.metadata().ok().map_or(0, |x| x.len());
        let mut command_list = String::with_capacity(capacity as usize);
        let _ = script.read_to_string(&mut command_list)?;
        if FAILURE == self.terminate_script_quotes(command_list.lines().map(|x| x.to_owned())) {
            self.previous_status = FAILURE;
        }
        Ok(self.previous_status)
    }

    /// A method for executing a function with the given `name`, using `args` as the input.
    /// If the function does not exist, an `IonError::DoesNotExist` is returned.
    pub fn execute_function(&mut self, name: &str, args: &[&str]) -> Result<i32, IonError> {
        self.functions
            .get_mut(name.into())
            .ok_or(IonError::DoesNotExist)
            .map(|fnc| fnc.clone())
            .and_then(|function| {
                function
                    .execute(self, args)
                    .map(|_| self.previous_status)
                    .map_err(|err| IonError::Function { why: err })
            })
    }

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
}

impl<'a> Expander for Shell {
    fn tilde(&self, input: &str) -> Option<String> {
        self.variables.tilde_expansion(input, &self.directory_stack)
    }

    /// Expand an array variable with some selection
    fn array(&self, array: &str, selection: Select) -> Option<Array> {
        let mut found = match self.variables.get_array(array) {
            Some(array) => match selection {
                Select::None => None,
                Select::All => Some(array.clone()),
                Select::Index(id) => id.resolve(array.len())
                    .and_then(|n| array.get(n))
                    .map(|x| Array::from_iter(Some(x.to_owned()))),
                Select::Range(range) => if let Some((start, length)) = range.bounds(array.len()) {
                    if array.len() <= start {
                        None
                    } else {
                        Some(
                            array
                                .iter()
                                .skip(start)
                                .take(length)
                                .map(|x| x.to_owned())
                                .collect::<Array>(),
                        )
                    }
                } else {
                    None
                },
                Select::Key(_) => None,
            },
            None => None,
        };
        if found.is_none() {
            found = match self.variables.get_map(array) {
                Some(map) => match selection {
                    Select::All => Some(
                        map.iter()
                            .map(|(_, value)| value.clone())
                            .collect::<Array>(),
                    ),
                    Select::Key(ref key) => {
                        Some(array![map.get(key.get()).unwrap_or(&"".into()).clone()])
                    }
                    _ => None,
                },
                None => None,
            }
        }
        found
    }
    /// Expand a string variable given if its quoted / unquoted
    fn variable(&self, variable: &str, quoted: bool) -> Option<Value> {
        use ascii_helpers::AsciiReplace;
        if quoted {
            self.get_var(variable)
        } else {
            self.get_var(variable)
                .map(|x| x.ascii_replace('\n', ' ').into())
        }
    }

    /// Uses a subshell to expand a given command.
    fn command(&self, command: &str) -> Option<Value> {
        let mut output = None;
        match self.fork(Capture::StdoutThenIgnoreStderr, move |shell| {
            shell.on_command(command)
        }) {
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
        output
    }
}
