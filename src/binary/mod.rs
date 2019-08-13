//! Contains the binary logic of Ion.
pub mod builtins;
mod completer;
mod designators;
mod history;
mod lexer;
mod prompt;
mod readln;

pub use completer::IonCompleter;
use ion_shell::{
    builtins::{man_pages, Status},
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

pub struct Builtins<'shell, 'context> {
    exit:          Box<dyn Fn(&[types::Str], &mut Shell<'_>) -> Status + 'shell>,
    exec:          Box<dyn Fn(&[types::Str], &mut Shell<'_>) -> Status + 'shell>,
    history:       Box<dyn Fn(&[types::Str], &mut Shell<'_>) -> Status + 'context>,
    set_huponexit: Box<dyn Fn(&[types::Str], &mut Shell<'_>) -> Status>,
    keybindings:   Box<dyn Fn(&[types::Str], &mut Shell<'_>) -> Status + 'context>,
}

pub struct InteractiveShell {
    terminated: Cell<bool>,
}

impl<'shell, 'context> Builtins<'shell, 'context> {
    pub fn new<'s, 'c>(
        shell: &Shell<'s>,
        context: Editor<IonCompleter<'c>>,
    ) -> (Rc<RefCell<Editor<IonCompleter<'context>>>>, Self)
    where
        's: 'shell,
        'c: 'context,
        'context: 'shell,
    {
        let huponexit = Rc::new(Cell::new(false));
        let huponexit_bis = huponexit.clone();
        let prep_for_exit = Rc::new(move |shell: &mut Shell<'_>| {
            // context will be sent a signal to commit all changes to the history file,
            // and waiting for the history thread in the background to finish.
            if huponexit_bis.get() {
                shell.resume_stopped();
                shell.background_send(Signal::SIGHUP).expect("Failed to prepare for exit");
            }
        });

        let prep_for_exit_bis = prep_for_exit.clone();
        let exit = shell.builtins().get("exit").unwrap();
        let exit = Box::new(move |args: &[types::Str], shell: &mut Shell<'_>| -> Status {
            prep_for_exit_bis(shell);
            exit(args, shell)
        });

        let prep_for_exit_bis = prep_for_exit.clone();
        let exec = shell.builtins().get("exec").unwrap();
        let exec = Box::new(move |args: &[types::Str], shell: &mut Shell<'_>| -> Status {
            prep_for_exit_bis(shell);
            exec(args, shell)
        });

        let context = Rc::new(RefCell::new(context));
        let context_bis = Rc::downgrade(&context);
        let history = Box::new(move |args: &[types::Str], _shell: &mut Shell<'_>| -> Status {
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
                    print!(
                        "{}",
                        context_bis.upgrade().unwrap().borrow().history().iter().format("\n")
                    );
                }
            }
            Status::SUCCESS
        });

        let set_huponexit =
            Box::new(move |args: &[types::Str], _shell: &mut Shell<'_>| -> Status {
                huponexit.set(match args.get(1).map(AsRef::as_ref) {
                    Some("false") | Some("off") => false,
                    _ => true,
                });
                Status::SUCCESS
            });

        // let context_bis = context.clone();
        let keybindings = Box::new(move |args: &[types::Str], _shell: &mut Shell<'_>| -> Status {
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
        });

        (context, Builtins { exec, exit, history, keybindings, set_huponexit })
    }
}

pub fn gen_context<'a>(_keybindings: KeyBindings) -> Editor<IonCompleter<'a>> { Editor::new() }

impl InteractiveShell {
    const CONFIG_FILE_NAME: &'static str = "initrc";

    pub fn new() -> Self { InteractiveShell { terminated: Cell::new(true) } }

    /// Handles commands given by the REPL, and saves them to history.
    pub fn save_command(&self, cmd: &str, context: &mut Editor<IonCompleter<'_>>) {
        let shell = context.helper_mut().unwrap().shell_mut();
        let tilde = shell.tilde(cmd).ok().map_or(false, |path| Path::new(&path.as_str()).is_dir());
        if !cmd.ends_with('/') && tilde {
            self.save_command_in_history(&[cmd, "/"].concat(), context);
        } else {
            self.save_command_in_history(cmd, context);
        }
    }

    pub fn add_callbacks<'b>(&self, context: &Rc<RefCell<Editor<IonCompleter<'b>>>>) {
        let mut ctx = context.borrow_mut();
        let shell = ctx.helper_mut().unwrap().shell_mut();

        let context = Rc::downgrade(&context);
        shell.set_on_command(Some(Box::new(move |shell, elapsed| {
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
                context.upgrade().unwrap().borrow_mut().history_mut().add(summary);
            }
        })));
    }

    fn create_config_file(base_dirs: &BaseDirectories) -> Result<(), io::Error> {
        let path = base_dirs.place_config_file(Self::CONFIG_FILE_NAME)?;
        OpenOptions::new().write(true).create_new(true).open(path).map(|_| ())
    }

    fn add_fns<'a, 'b>(context: &mut Editor<IonCompleter<'b>>, builtins: &'b Builtins<'a, 'b>) {
        let shell = context.helper_mut().unwrap().shell_mut();
        shell
            .builtins_mut()
            .add("history", &builtins.history, "Display a log of all commands previously executed")
            .add("keybindings", &builtins.keybindings, "Change the keybindings")
            .add("exit", &builtins.exit, "Exits the current session")
            .add("exec", &builtins.exec, "Replace the shell with the given command.")
            .add(
                "huponexit",
                &builtins.set_huponexit,
                "Hangup the shell's background jobs on exit",
            );
    }

    /// Creates an interactive session that reads from a prompt provided by
    /// Liner.
    pub fn execute_interactive<'a, 'b, 'c, 'd>(
        &'c self,
        builtins: &'c Builtins<'d, 'b>,
        context: Rc<RefCell<Editor<IonCompleter<'b>>>>,
    ) -> !
    where
        'd: 'b,
    {
        // Downgrading the lifetime should be fine, as all the references to it are contained
        // within the context, which dies in this scope.
        let context =
            unsafe { std::mem::transmute::<_, Rc<RefCell<Editor<IonCompleter<'c>>>>>(context) };
        Self::add_fns(&mut context.borrow_mut(), builtins);
        self.add_callbacks(&context);

        let mut ctx = context.borrow_mut();
        let shell = ctx.helper_mut().unwrap().shell_mut();
        match BaseDirectories::with_prefix("ion") {
            Ok(project_dir) => {
                Self::exec_init_file(&project_dir, shell);
                Self::load_history(&project_dir, &mut ctx);
            }
            Err(err) => eprintln!("ion: unable to get xdg base directory: {}", err),
        }

        std::mem::drop(ctx);
        self.exec(context);
    }

    fn load_history(project_dir: &BaseDirectories, editor: &mut Editor<IonCompleter<'_>>) {
        let shell = editor.helper_mut().unwrap().shell_mut();

        shell.variables_mut().set("HISTFILE_ENABLED", "1");

        // History Timestamps enabled variable, disabled by default
        shell.variables_mut().set("HISTORY_TIMESTAMP", "0");
        shell
            .variables_mut()
            .set("HISTORY_IGNORE", array!["no_such_command", "whitespace", "duplicates"]);
        // Initialize the HISTFILE variable
        if let Some(histfile) = project_dir.find_data_file("history") {
            shell.variables_mut().set("HISTFILE", histfile.to_string_lossy().as_ref());
            let _ = editor.history_mut().load(&histfile);
        } else {
            match project_dir.place_data_file("history") {
                Ok(histfile) => {
                    eprintln!("ion: creating history file at \"{}\"", histfile.display());
                    shell.variables_mut().set("HISTFILE", histfile.to_string_lossy().as_ref());
                    let _ = editor.history_mut().load(&histfile);
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

    fn exec<'b, 'c, 'd>(&'c self, context: Rc<RefCell<Editor<IonCompleter<'b>>>>) -> ! {
        // A reference would be enough for the completer, I however did not find a way to prove it
        // to the compiler with lifetimes, as the context itself exists for longer that the actual
        // completer
        loop {
            if let Err(err) = io::stdout().flush() {
                eprintln!("ion: failed to flush stdout: {}", err);
            }
            if let Err(err) = io::stderr().flush() {
                println!("ion: failed to flush stderr: {}", err);
            }
            let mut lines = std::iter::from_fn(|| self.readln(&mut context.borrow_mut()))
                .flat_map(|s| s.into_bytes().into_iter().chain(Some(b'\n')));
            match Terminator::new(&mut lines).terminate() {
                Some(command) => {
                    let cmd: &str = &designators::expand_designators(
                        &context.borrow().history(),
                        command.trim_end(),
                    );
                    self.terminated.set(true);
                    {
                        let mut context = context.borrow_mut();
                        let helper = context.helper_mut().unwrap();
                        let shell = helper.shell_mut();
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
                        self.save_command(&cmd, &mut context);
                    }
                }
                None => self.terminated.set(true),
            }
        }
    }
}
