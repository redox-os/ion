//! Contains the binary logic of Ion.
pub mod builtins;
mod completer;
mod designators;
mod history;
mod lexer;
mod prompt;
mod readln;

use ion_shell::{
    builtins::{man_pages, BuiltinFunction, Status},
    expansion::Expander,
    parser::Terminator,
    types::{self, array},
    IonError, PipelineError, Shell, Signal, Value,
};
use itertools::Itertools;
use liner::{Buffer, Context, KeyBindings};
use std::{
    cell::{Cell, RefCell},
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::io::{AsRawFd, IntoRawFd},
    path::Path,
    rc::Rc,
};
use xdg::BaseDirectories;

#[cfg(not(feature = "advanced_arg_parsing"))]
pub const MAN_ION: &str = r#"ion 1.0.0-alpha
The fast, safe, modern rust shell. Ion is a commandline shell created to be a faster and easier to use alternative to
the currently available shells. It is not POSIX compliant.

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
    -o <key-bindings>        Shortcut layout. Valid options: "vi", "emacs"

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

pub struct InteractiveShell<'a> {
    context:    Rc<RefCell<Context>>,
    shell:      RefCell<Shell<'a>>,
    terminated: Cell<bool>,
    huponexit:  Rc<Cell<bool>>,
}

impl<'a> InteractiveShell<'a> {
    const CONFIG_FILE_NAME: &'static str = "initrc";

    pub fn new(shell: Shell<'a>) -> Self {
        let mut context = Context::new();
        context.word_divider_fn = Box::new(word_divide);
        InteractiveShell {
            context:    Rc::new(RefCell::new(context)),
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
                let summary =
                    format!("#summary# elapsed real time: {:.9} seconds", elapsed.as_secs_f32(),);
                println!("{}", summary);
                context.borrow_mut().history.push(summary.into()).unwrap_or_else(|err| {
                    eprintln!("ion: history append: {}", err);
                });
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
            context_bis.borrow_mut().history.commit_to_file();
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
                Some("+inc_append") => {
                    context_bis.borrow_mut().history.inc_append = true;
                }
                Some("-inc_append") => {
                    context_bis.borrow_mut().history.inc_append = false;
                }
                Some("+share") => {
                    context_bis.borrow_mut().history.inc_append = true;
                    context_bis.borrow_mut().history.share = true;
                }
                Some("-share") => {
                    context_bis.borrow_mut().history.inc_append = false;
                    context_bis.borrow_mut().history.share = false;
                }
                Some("+duplicates") => {
                    context_bis.borrow_mut().history.load_duplicates = true;
                }
                Some("-duplicates") => {
                    context_bis.borrow_mut().history.load_duplicates = false;
                }
                Some(_) => {
                    Status::error(
                        "Invalid history option. Choices are [+|-] inc_append, duplicates and \
                         share (implies inc_append).",
                    );
                }
                None => {
                    print!("{}", context_bis.borrow().history.buffers.iter().format("\n"));
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
                Some("vi") => {
                    context_bis.borrow_mut().key_bindings = KeyBindings::Vi;
                    Status::SUCCESS
                }
                Some("emacs") => {
                    context_bis.borrow_mut().key_bindings = KeyBindings::Emacs;
                    Status::SUCCESS
                }
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

    fn load_history(project_dir: &BaseDirectories, shell: &mut Shell, context: &mut Context) {
        shell.variables_mut().set("HISTFILE_ENABLED", "1");

        // History Timestamps enabled variable, disabled by default
        shell.variables_mut().set("HISTORY_TIMESTAMP", "0");
        shell
            .variables_mut()
            .set("HISTORY_IGNORE", array!["no_such_command", "whitespace", "duplicates"]);
        // Initialize the HISTFILE variable
        if let Some(histfile) = project_dir.find_data_file("history") {
            shell.variables_mut().set("HISTFILE", histfile.to_string_lossy().as_ref());
            let _ = context.history.set_file_name_and_load_history(&histfile);
        } else {
            match project_dir.place_data_file("history") {
                Ok(histfile) => {
                    eprintln!("ion: creating history file at \"{}\"", histfile.display());
                    shell.variables_mut().set("HISTFILE", histfile.to_string_lossy().as_ref());
                    let _ = context.history.set_file_name_and_load_history(&histfile);
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

    fn exec_single_command(&mut self, command: &str) {
        let cmd: &str =
            &designators::expand_designators(&self.context.borrow(), command.trim_end());
        self.terminated.set(true);
        {
            let mut shell = self.shell.borrow_mut();
            match shell.on_command(&cmd, true) {
                Ok(_) => (),
                Err(IonError::PipelineExecutionError(PipelineError::CommandNotFound(command))) => {
                    if Self::try_cd(&command, &mut shell).ok().map_or(false, |res| res.is_failure())
                    {
                        if let Some(Value::Function(func)) =
                            shell.variables().get("COMMAND_NOT_FOUND").cloned()
                        {
                            if let Err(why) = shell.execute_function(&func, &["ion", &command]) {
                                eprintln!("ion: command not found handler: {}", why);
                            }
                        } else {
                            eprintln!("ion: command not found: {}", command);
                        }
                    }
                    // Status::COULD_NOT_EXEC
                }
                Err(IonError::PipelineExecutionError(PipelineError::CommandExecError(
                    ref err,
                    ref command,
                ))) if err.kind() == io::ErrorKind::PermissionDenied && command.len() == 1 => {
                    if Self::try_cd(&command[0], &mut shell)
                        .ok()
                        .map_or(false, |res| res.is_failure())
                    {
                        eprintln!("ion: {}", err);
                        shell.reset_flow();
                    }
                }
                Err(err) => {
                    eprintln!("ion: {}", err);
                    shell.reset_flow();
                }
            }
        }
        self.save_command(&cmd);
    }

    fn exec<T: Fn(&mut Shell<'_>)>(mut self, prep_for_exit: &T) -> ! {
        loop {
            if let Err(err) = io::stdout().flush() {
                eprintln!("ion: failed to flush stdio: {}", err);
            }
            if let Err(err) = io::stderr().flush() {
                println!("ion: failed to flush stderr: {}", err);
            }
            match self.readln(prep_for_exit) {
                Some(lines) => {
                    for command in lines
                        .into_bytes()
                        .into_iter()
                        .batching(|bytes| Terminator::new(bytes).terminate())
                    {
                        self.exec_single_command(&command);
                    }
                }
                None => self.terminated.set(true),
            }
        }
    }

    /// Try to cd if the command failed
    fn try_cd(dir: &str, shell: &mut Shell<'_>) -> nix::Result<Status> {
        // Gag the cd output
        let null = OpenOptions::new()
            .write(true)
            .open(if cfg!(target_os = "redox") { "null:" } else { "/dev/null" })
            .map_err(|err| {
                nix::Error::from_errno(nix::errno::Errno::from_i32(err.raw_os_error().unwrap()))
            })?
            .into_raw_fd();

        let fd = io::stderr().as_raw_fd();
        let fd_dup = nix::unistd::dup(fd)?;
        nix::unistd::dup2(null, fd)?;
        let out = ion_shell::builtins::builtin_cd(&["cd".into(), dir.into()], shell);
        nix::unistd::dup2(fd_dup, fd)?;
        nix::unistd::close(fd_dup)?;
        Ok(out)
    }

    /// Set the keybindings of the underlying liner context
    pub fn set_keybindings(&mut self, key_bindings: KeyBindings) {
        self.context.borrow_mut().key_bindings = key_bindings;
    }
}

#[derive(Debug)]
struct WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    iter:       I,
    count:      usize,
    word_start: Option<usize>,
}
impl<I> WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    #[inline]
    fn check_boundary(&mut self, c: char, index: usize, escaped: bool) -> Option<(usize, usize)> {
        if let Some(start) = self.word_start {
            if c == ' ' && !escaped {
                self.word_start = None;
                Some((start, index))
            } else {
                self.next()
            }
        } else {
            if c != ' ' {
                self.word_start = Some(index);
            }
            self.next()
        }
    }
}
impl<I> Iterator for WordDivide<I>
where
    I: Iterator<Item = (usize, char)>,
{
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        self.count += 1;
        match self.iter.next() {
            Some((i, '\\')) => {
                if let Some((_, cnext)) = self.iter.next() {
                    self.count += 1;
                    // We use `i` in order to include the backslash as part of the word
                    self.check_boundary(cnext, i, true)
                } else {
                    self.next()
                }
            }
            Some((i, c)) => self.check_boundary(c, i, false),
            None => {
                // When start has been set, that means we have encountered a full word.
                self.word_start.take().map(|start| (start, self.count - 1))
            }
        }
    }
}

fn word_divide(buf: &Buffer) -> Vec<(usize, usize)> {
    // -> impl Iterator<Item = (usize, usize)> + 'a
    WordDivide { iter: buf.chars().copied().enumerate(), count: 0, word_start: None }.collect()
    // TODO: return iterator directly :D
}
