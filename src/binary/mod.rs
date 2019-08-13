//! Contains the binary logic of Ion.
pub mod builtins;
mod completer;
mod designators;
mod history;
mod lexer;
mod prompt;
mod readln;

use self::completer::IonCompleter;
use ion_shell::{
    builtins::{man_pages, BuiltinFunction, Status},
    expansion::Expander,
    parser::Terminator,
    types::{self, array},
    IonError, PipelineError, Shell, Signal, Value,
};
use itertools::Itertools;
use rustyline::Editor;
use std::{
    cell::{Cell, RefCell},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    rc::Rc,
};
use xdg::BaseDirectories;

#[cfg(not(feature = "advanced_arg_parsing"))]
pub const MAN_ION: &str = r#"Ion - The Ion Shell 1.0.0-alpha
Ion is a commandline shell created to be a faster and easier to use alternative to the currently available shells. It is
not POSIX compliant.

USAGE:
    ion [FLAGS] [OPTIONS] [args]...

FLAGS:
    -f, --fake-interactive    Use a fake interactive mode, where errors don't exit the shell
    -h, --help                Prints help information
    -i, --interactive         Force interactive mode
    -n, --no-execute          Do not execute any commands, perform only syntax checking
    -x                        Print commands before execution
    -v, --version             Print the version, platform and revision of Ion then exit

OPTIONS:
    -c <command>             Evaluate given commands instead of reading from the commandline
    -o <key_bindings>        Shortcut layout. Valid options: "vi", "emacs"

ARGS:
    <args>...    Script arguments (@args). If the -c option is not specified, the first parameter is taken as a
                 filename to execute"#;

pub(crate) const MAN_HISTORY: &str = r#"NAME
    history - print command history

SYNOPSIS
    history [option]

DESCRIPTION
    Prints or manupulate the command history.

OPTIONS:
    +inc_append: Append each command to history as entered.
    -inc_append: Default, do not append each command to history as entered.
    +shared: Share history between shells using the same history file, implies inc_append.
    -shared: Default, do not share shell history.
    +duplicates: Default, allow duplicates in history.
    -duplicates: Do not allow duplicates in history.
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyBindings {
    Vi,
    Emacs,
}

pub struct InteractiveShell<'a> {
    context:    Rc<RefCell<Editor<IonCompleter<'a>>>>,
    shell:      RefCell<Shell<'a>>,
    terminated: Cell<bool>,
    huponexit:  Rc<Cell<bool>>,
}

impl<'a> InteractiveShell<'a> {
    const CONFIG_FILE_NAME: &'static str = "initrc";

    pub fn new(shell: Shell<'a>, _keybindings: KeyBindings) -> Self {
        InteractiveShell {
            context:    Rc::new(RefCell::new(Editor::new())),
            shell:      RefCell::new(shell),
            terminated: Cell::new(true),
            huponexit:  Rc::new(Cell::new(false)),
        }
    }

    /// Handles commands given by the REPL, and saves them to history.
    pub fn save_command(&self, cmd: &str) {
        if !cmd.ends_with('/')
            && self
                .shell
                .borrow()
                .tilde(cmd)
                .ok()
                .map_or(false, |path| Path::new(&path.as_str()).is_dir())
        {
            self.save_command_in_history(&[cmd, "/"].concat());
        } else {
            self.save_command_in_history(cmd);
        }
    }

    pub fn add_callbacks(&self) {
        let context = self.context.clone();
        self.shell.borrow_mut().set_on_command(Some(Box::new(move |shell, elapsed| {
            // If `RECORD_SUMMARY` is set to "1" (True, Yes), then write a summary of the
            // pipline just executed to the the file and context histories. At the
            // moment, this means record how long it took.
            if Some("1".into()) == shell.variables().get_str("RECORD_SUMMARY").ok() {
                let summary = format!(
                    "#summary# elapsed real time: {}.{:09} seconds",
                    elapsed.as_secs(),
                    elapsed.subsec_nanos()
                );
                println!("{}", summary);
                context.borrow_mut().history_mut().add(summary);
            }
        })));
    }

    fn create_config_file(base_dirs: &BaseDirectories) -> Result<(), io::Error> {
        let path = base_dirs.place_config_file(Self::CONFIG_FILE_NAME)?;
        OpenOptions::new().write(true).create_new(true).open(path).map(|_| ())
    }

    /// Creates an interactive session that reads from a prompt provided by
    /// Liner.
    pub fn execute_interactive(self) -> ! {
        let context_bis = self.context.clone();
        let huponexit = self.huponexit.clone();
        let prep_for_exit = &move |shell: &mut Shell<'_>| {
            // context will be sent a signal to commit all changes to the history file,
            // and waiting for the history thread in the background to finish.
            if huponexit.get() {
                shell.resume_stopped();
                shell.background_send(Signal::SIGHUP).expect("Failed to prepare for exit");
            }
            if let Some(Value::Str(histfile)) = shell.variables().get("HISTFILE") {
                context_bis.borrow_mut().history().save(histfile.as_str());
            }
        };

        let exit = self.shell.borrow().builtins().get("exit").unwrap();
        let exit = &|args: &[types::Str], shell: &mut Shell<'_>| -> Status {
            prep_for_exit(shell);
            exit(args, shell)
        };

        let exec = self.shell.borrow().builtins().get("exec").unwrap();
        let exec = &|args: &[types::Str], shell: &mut Shell<'_>| -> Status {
            prep_for_exit(shell);
            exec(args, shell)
        };

        let context_bis = self.context.clone();
        let history = &move |args: &[types::Str], _shell: &mut Shell<'_>| -> Status {
            if man_pages::check_help(args, MAN_HISTORY) {
                return Status::SUCCESS;
            }

            match args.get(1).map(|s| s.as_str()) {
                // Some("+inc_append") => {
                // context_bis.borrow_mut().history.inc_append = true;
                // }
                // Some("-inc_append") => {
                // context_bis.borrow_mut().history.inc_append = false;
                // }
                // Some("+share") => {
                // context_bis.borrow_mut().history.inc_append = true;
                // context_bis.borrow_mut().history.share = true;
                // }
                // Some("-share") => {
                // context_bis.borrow_mut().history.inc_append = false;
                // context_bis.borrow_mut().history.share = false;
                // }
                // Some("+duplicates") => {
                // context_bis.borrow_mut().history.load_duplicates = true;
                // }
                // Some("-duplicates") => {
                // context_bis.borrow_mut().history.load_duplicates = false;
                // }
                Some(_) => {
                    Status::error(
                        "Invalid history option. Choices are [+|-] inc_append, duplicates and \
                         share (implies inc_append).",
                    );
                }
                None => {
                    print!("{}", context_bis.borrow().history().iter().format("\n"));
                }
            }
            Status::SUCCESS
        };

        let huponexit = self.huponexit.clone();
        let set_huponexit: BuiltinFunction = &move |args, _shell| {
            huponexit.set(match args.get(1).map(AsRef::as_ref) {
                Some("false") | Some("off") => false,
                _ => true,
            });
            Status::SUCCESS
        };

        let context_bis = self.context.clone();
        let keybindings = &move |args: &[types::Str], _shell: &mut Shell<'_>| -> Status {
            match args.get(1).map(|s| s.as_str()) {
                // Some("vi") => {
                // context_bis.borrow_mut().key_bindings = KeyBindings::Vi;
                // Status::SUCCESS
                // }
                // Some("emacs") => {
                // context_bis.borrow_mut().key_bindings = KeyBindings::Emacs;
                // Status::SUCCESS
                // }
                Some(_) => Status::error("Invalid keybindings. Choices are vi and emacs"),
                None => Status::error("keybindings need an argument"),
            }
        };

        // change the lifetime to allow adding local builtins
        let InteractiveShell { context, shell, terminated, huponexit } = self;
        let mut shell = shell.into_inner();
        shell
            .builtins_mut()
            .add("history", history, "Display a log of all commands previously executed")
            .add("keybindings", keybindings, "Change the keybindings")
            .add("exit", exit, "Exits the current session")
            .add("exec", exec, "Replace the shell with the given command.")
            .add("huponexit", set_huponexit, "Hangup the shell's background jobs on exit");

        match BaseDirectories::with_prefix("ion") {
            Ok(project_dir) => {
                Self::exec_init_file(&project_dir, &mut shell);
                Self::load_history(&project_dir, &mut shell, &mut context.borrow_mut());
            }
            Err(err) => eprintln!("ion: unable to get xdg base directory: {}", err),
        }

        InteractiveShell { context, shell: RefCell::new(shell), terminated, huponexit }
            .exec(prep_for_exit)
    }

    fn load_history(
        project_dir: &BaseDirectories,
        shell: &mut Shell,
        ed: &mut Editor<IonCompleter<'a>>,
    ) {
        shell.variables_mut().set("HISTFILE_ENABLED", "1");

        // History Timestamps enabled variable, disabled by default
        shell.variables_mut().set("HISTORY_TIMESTAMP", "0");
        shell
            .variables_mut()
            .set("HISTORY_IGNORE", array!["no_such_command", "whitespace", "duplicates"]);
        // Initialize the HISTFILE variable
        if let Some(histfile) = project_dir.find_data_file("history") {
            shell.variables_mut().set("HISTFILE", histfile.to_string_lossy().as_ref());
            let _ = ed.history_mut().load(&histfile);
        } else {
            match project_dir.place_data_file("history") {
                Ok(histfile) => {
                    eprintln!("ion: creating history file at \"{}\"", histfile.display());
                    shell.variables_mut().set("HISTFILE", histfile.to_string_lossy().as_ref());
                    let _ = ed.history_mut().load(&histfile);
                }
                Err(err) => println!("ion: could not create history file: {}", err),
            }
        }
    }

    fn exec_init_file(project_dir: &BaseDirectories, shell: &mut Shell) {
        let initrc = project_dir.find_config_file(Self::CONFIG_FILE_NAME);
        match initrc.and_then(|initrc| fs::File::open(&initrc).ok()) {
            Some(script) => {
                if let Err(err) = shell.execute_command(std::io::BufReader::new(script)) {
                    eprintln!("ion: could not exec initrc: {}", err);
                }
            }
            None => {
                if let Err(err) = Self::create_config_file(project_dir) {
                    eprintln!("ion: could not create config file: {}", err);
                }
            }
        }
    }

    fn exec<T: Fn(&mut Shell<'_>)>(self, prep_for_exit: &T) -> ! {
        loop {
            if let Err(err) = io::stdout().flush() {
                eprintln!("ion: failed to flush stdio: {}", err);
            }
            if let Err(err) = io::stderr().flush() {
                println!("ion: failed to flush stderr: {}", err);
            }
            let mut lines = std::iter::from_fn(|| self.readln(prep_for_exit))
                .flat_map(|s| s.into_bytes().into_iter().chain(Some(b'\n')));
            match Terminator::new(&mut lines).terminate() {
                Some(command) => {
                    let cmd: &str = &designators::expand_designators(
                        &self.context.borrow().history(),
                        command.trim_end(),
                    );
                    self.terminated.set(true);
                    {
                        let mut shell = self.shell.borrow_mut();
                        match shell.on_command(&cmd) {
                            Ok(_) => (),
                            Err(IonError::PipelineExecutionError(
                                PipelineError::CommandNotFound(command),
                            )) => {
                                if let Some(Value::Function(func)) =
                                    shell.variables().get("COMMAND_NOT_FOUND").cloned()
                                {
                                    if let Err(why) =
                                        shell.execute_function(&func, &["ion", &command])
                                    {
                                        eprintln!("ion: command not found handler: {}", why);
                                    }
                                } else {
                                    eprintln!("ion: command not found: {}", command);
                                }
                                // Status::COULD_NOT_EXEC
                            }
                            Err(err) => {
                                eprintln!("ion: {}", err);
                                shell.reset_flow();
                            }
                        }
                    }
                    self.save_command(&cmd);
                }
                None => self.terminated.set(true),
            }
        }
    }
}
