mod assignments;
mod binary;
mod completer;
mod flow;
mod history;
mod job;
mod pipe_exec;
pub mod colors;
pub mod directory_stack;
pub mod flags;
pub mod plugins;
pub mod flow_control;
pub mod signals;
pub mod status;
pub mod variables;

pub(crate) use self::binary::Binary;
pub(crate) use self::flow::FlowLogic;
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
use app_dirs::{app_root, AppDataType, AppInfo};
use builtins::*;
use fnv::FnvHashMap;
use liner::Context;
use parser::{ArgumentSplitter, Expander, Select};
use parser::pipelines::Pipeline;
use smallvec::SmallVec;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::ops::Deref;
use std::process;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use types::*;

/// The shell structure is a megastructure that manages all of the state of the shell throughout
/// the entirety of the
/// program. It is initialized at the beginning of the program, and lives until the end of the
/// program.
pub(crate) struct Shell<'a> {
    /// Contains a list of built-in commands that were created when the program started.
    pub builtins: &'a FnvHashMap<&'static str, Builtin>,
    /// Contains the history, completions, and manages writes to the history file.
    /// Note that the context is only available in an interactive session.
    pub context: Option<Context>,
    /// Contains the aliases, strings, and array variable maps.
    pub variables: Variables,
    /// Contains the current state of flow control parameters.
    flow_control: FlowControl,
    /// Contains the directory stack parameters.
    pub directory_stack: DirectoryStack,
    /// Contains all of the user-defined functions that have been created.
    pub functions: FnvHashMap<Identifier, Function>,
    /// When a command is executed, the final result of that command is stored here.
    pub previous_status: i32,
    /// The job ID of the previous command sent to the background.
    pub previous_job: u32,
    /// Contains all the boolean flags that control shell behavior.
    pub flags: u8,
    /// A temporary field for storing foreground PIDs used by the pipeline execution.
    foreground: Vec<u32>,
    /// Contains information on all of the active background processes that are being managed
    /// by the shell.
    pub background: Arc<Mutex<Vec<BackgroundProcess>>>,
    /// If set, denotes that this shell is running as a background job.
    pub is_background_shell: bool,
    /// Set when a signal is received, this will tell the flow control logic to abort.
    pub break_flow: bool,
    /// When the `fg` command is run, this will be used to communicate with the specified
    /// background process.
    pub foreground_signals: Arc<ForegroundSignals>,
    /// Stores the patterns used to determine whether a command should be saved in the history
    /// or not
    ignore_setting: IgnoreSetting,
}

impl<'a> Shell<'a> {
    /// Panics if DirectoryStack construction fails
    pub(crate) fn new(builtins: &'a FnvHashMap<&'static str, Builtin>) -> Shell<'a> {
        Shell {
            builtins:            builtins,
            context:             None,
            variables:           Variables::default(),
            flow_control:        FlowControl::default(),
            directory_stack:     DirectoryStack::new(),
            functions:           FnvHashMap::default(),
            previous_job:        !0,
            previous_status:     0,
            flags:               0,
            foreground:          Vec::new(),
            background:          Arc::new(Mutex::new(Vec::new())),
            is_background_shell: false,
            break_flow:          false,
            foreground_signals:  Arc::new(ForegroundSignals::new()),
            ignore_setting:      IgnoreSetting::default(),
        }
    }

    pub(crate) fn next_signal(&self) -> Option<i32> {
        for sig in 0..32 {
            if signals::PENDING.fetch_and(!(1 << sig), Ordering::SeqCst) & (1 << sig) == 1 << sig {
                return Some(sig);
            }
        }

        None
    }

    pub(crate) fn exit(&mut self, status: i32) -> ! {
        if let Some(context) = self.context.as_mut() {
            context.history.commit_history();
        }
        process::exit(status);
    }

    /// This function updates variables that need to be kept consistent with each iteration
    /// of the prompt. For example, the PWD variable needs to be updated to reflect changes to
    /// the
    /// the current working directory.
    fn update_variables(&mut self) {
        // Update the PWD (Present Working Directory) variable if the current working directory has
        // been updated.
        env::current_dir().ok().map_or_else(
            || env::set_var("PWD", "?"),
            |path| {
                let pwd = self.variables.get_var_or_empty("PWD");
                let pwd: &str = &pwd;
                let current_dir = path.to_str().unwrap_or("?");
                if pwd != current_dir {
                    env::set_var("OLDPWD", pwd);
                    env::set_var("PWD", current_dir);
                }
            },
        )
    }

    /// Evaluates the source init file in the user's home directory.
    pub(crate) fn evaluate_init_file(&mut self) {
        match app_root(
            AppDataType::UserConfig,
            &AppInfo {
                name:   "ion",
                author: "Redox OS Developers",
            },
        ) {
            Ok(mut initrc) => {
                initrc.push("initrc");
                if initrc.exists() {
                    self.execute_script(&initrc);
                } else {
                    eprintln!("ion: creating initrc file at {:?}", initrc);
                    if let Err(why) = File::create(initrc) {
                        eprintln!("ion: could not create initrc file: {}", why);
                    }
                }
            }
            Err(why) => {
                eprintln!("ion: unable to get config root: {}", why);
            }
        }
    }

    /// Executes a pipeline and returns the final exit status of the pipeline.
    /// To avoid infinite recursion when using aliases, the noalias boolean will be set the true
    /// if an alias branch was executed.
    fn run_pipeline(&mut self, pipeline: &mut Pipeline) -> Option<i32> {
        let command_start_time = SystemTime::now();
        let builtins = self.builtins;

        // Expand any aliases found
        for job_no in 0..pipeline.jobs.len() {
            if let Some(alias) = {
                let key: &str = pipeline.jobs[job_no].command.as_ref();
                self.variables.aliases.get(key)
            } {
                let new_args = ArgumentSplitter::new(alias)
                    .map(String::from)
                    .chain(pipeline.jobs[job_no].args.drain().skip(1))
                    .collect::<SmallVec<[String; 4]>>();
                pipeline.jobs[job_no].command = new_args[0].clone().into();
                pipeline.jobs[job_no].args = new_args;
            }
        }

        // Branch if -> input == shell command i.e. echo
        let exit_status = if let Some(command) = {
            let key: &str = pipeline.jobs[0].command.as_ref();
            builtins.get(key)
        } {
            pipeline.expand(self);
            // Run the 'main' of the command and set exit_status
            if !pipeline.requires_piping() {
                if self.flags & PRINT_COMMS != 0 {
                    eprintln!("> {}", pipeline.to_string());
                }
                let borrowed = &pipeline.jobs[0].args;
                let small: SmallVec<[&str; 4]> = borrowed.iter().map(|x| x as &str).collect();
                Some((command.main)(&small, self))
            } else {
                Some(self.execute_pipeline(pipeline))
            }
        // Branch else if -> input == shell function and set the exit_status
        } else if let Some(function) = self.functions.get(&pipeline.jobs[0].command).cloned() {
            if !pipeline.requires_piping() {
                let args: &[String] = pipeline.jobs[0].args.deref();
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
                            expected_type,
                            value
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

        // If `RECORD_SUMMARY` is set to "1" (True, Yes), then write a summary of the pipline
        // just executed to the the file and context histories. At the moment, this means
        // record how long it took.
        if let Some(context) = self.context.as_mut() {
            if "1" == self.variables.get_var_or_empty("RECORD_SUMMARY") {
                if let Ok(elapsed_time) = command_start_time.elapsed() {
                    let summary = format!(
                        "#summary# elapsed real time: {}.{:09} seconds",
                        elapsed_time.as_secs(),
                        elapsed_time.subsec_nanos()
                    );
                    context.history.push(summary.into()).unwrap_or_else(|err| {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = writeln!(stderr, "ion: {}\n", err);
                    });
                }
            }
        }

        // Retrieve the exit_status and set the $? variable and history.previous_status
        if let Some(code) = exit_status {
            self.variables.set_var("?", &code.to_string());
            self.previous_status = code;
        }
        exit_status
    }
}

impl<'a> Expander for Shell<'a> {
    fn tilde(&self, input: &str) -> Option<String> {
        self.variables.tilde_expansion(input, &self.directory_stack)
    }

    /// Expand an array variable with some selection
    fn array(&self, array: &str, selection: Select) -> Option<Array> {
        use std::iter::FromIterator;
        let mut found = match self.variables.get_array(array) {
            Some(array) => match selection {
                Select::None => None,
                Select::All => Some(array.clone()),
                Select::Index(id) => id.resolve(array.len())
                    .and_then(|n| array.get(n))
                    .map(|x| Array::from_iter(Some(x.to_owned()))),
                Select::Range(range) => if let Some((start, length)) = range.bounds(array.len()) {
                    let array = array
                        .iter()
                        .skip(start)
                        .take(length)
                        .map(|x| x.to_owned())
                        .collect::<Array>();
                    if array.is_empty() {
                        None
                    } else {
                        Some(array)
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
                    Select::All => {
                        let mut arr = Array::new();
                        for (_, value) in map {
                            arr.push(value.clone());
                        }
                        Some(arr)
                    }
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
            self.variables.get_var(variable)
        } else {
            self.variables.get_var(variable).map(|x| x.ascii_replace('\n', ' ').into())
        }
    }
    /// Expand a subshell expression
    fn command(&self, command: &str) -> Option<Value> { self.variables.command_expansion(command) }
}
