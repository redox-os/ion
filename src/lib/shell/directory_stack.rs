use super::status::{FAILURE, SUCCESS};
use super::variables::Variables;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::env::{current_dir, home_dir, set_current_dir};
use std::path::PathBuf;

pub struct DirectoryStack {
    dirs: VecDeque<PathBuf>, // The top is always the current directory
}

impl DirectoryStack {
    /// Create a new `DirectoryStack` containing the current working directory,
    /// if available.
    pub(crate) fn new() -> DirectoryStack {
        let mut dirs: VecDeque<PathBuf> = VecDeque::new();
        match current_dir() {
            Ok(curr_dir) => {
                dirs.push_front(curr_dir);
                DirectoryStack { dirs: dirs }
            }
            Err(_) => {
                eprintln!("ion: failed to get current directory when building directory stack");
                DirectoryStack { dirs: dirs }
            }
        }
    }

    /// This function will take a map of variables as input and attempt to parse the value of
    /// the
    /// directory stack size variable. If it succeeds, it will return the value of that
    /// variable,
    /// else it will return a default value of 1000.
    fn get_size(variables: &Variables) -> usize {
        variables
            .get_var_or_empty("DIRECTORY_STACK_SIZE")
            .parse::<usize>()
            .unwrap_or(1000)
    }

    /// Attempts to set the current directory to the directory stack's previous directory,
    /// and then removes the front directory from the stack.
    pub(crate) fn popd<I: IntoIterator>(&mut self, args: I) -> Result<(), Cow<'static, str>>
    where
        I::Item: AsRef<str>,
    {
        let mut keep_front = false; // whether the -n option is present
        let mut count_from_front = true; // <=> input number is positive
        let mut num: usize = 0;

        for arg in args.into_iter().skip(1) {
            let arg = arg.as_ref();
            if arg == "-n" {
                keep_front = true;
            } else {
                match parse_numeric_arg(arg) {
                    Some((x, y)) => {
                        count_from_front = x;
                        num = y;
                    }
                    None => {
                        return Err(Cow::Owned(format!(
                            "ion: popd: {}: invalid argument\n",
                            arg
                        )))
                    }
                };
            }
        }

        let len: usize = self.dirs.len();
        if len <= 1 {
            return Err(Cow::Borrowed("ion: popd: directory stack empty\n"));
        }

        let mut index: usize = if count_from_front {
            num
        } else {
            (len - 1).checked_sub(num).ok_or_else(|| {
                Cow::Owned(format!(
                    "ion: popd: negative directory stack index out of range\n"
                ))
            })?
        };

        // apply -n
        if index == 0 && keep_front {
            index = 1;
        }

        // change to new directory, return if not possible
        if index == 0 {
            self.set_current_dir_by_index(1, "popd")?;
        }

        // pop element
        if self.dirs.remove(index).is_none() {
            return Err(Cow::Owned(format!(
                "ion: popd: {}: directory stack index out of range\n",
                index
            )));
        }

        self.print_dirs();
        Ok(())
    }

    pub(crate) fn pushd<I: IntoIterator>(
        &mut self,
        args: I,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>>
    where
        I::Item: AsRef<str>,
    {
        enum Action {
            Switch,          // <no arguments>
            RotLeft(usize),  // +[num]
            RotRight(usize), // -[num]
            Push(PathBuf),   // [dir]
        }

        let mut keep_front = false; // whether the -n option is present
        let mut action: Action = Action::Switch;

        for arg in args.into_iter().skip(1) {
            let arg = arg.as_ref();
            if arg == "-n" {
                keep_front = true;
            } else if let Action::Switch = action {
                // if action is not yet defined
                action = match parse_numeric_arg(arg) {
                    Some((true, num)) => Action::RotLeft(num),
                    Some((false, num)) => Action::RotRight(num),
                    None => Action::Push(PathBuf::from(arg)), // no numeric arg => `dir`-parameter
                };
            } else {
                return Err(Cow::Borrowed("ion: pushd: too many arguments\n"));
            }
        }

        let len = self.dirs.len();
        match action {
            Action::Switch => {
                if len < 2 {
                    return Err(Cow::Borrowed("ion: pushd: no other directory\n"));
                }
                if !keep_front {
                    self.set_current_dir_by_index(1, "pushd")?;
                    self.dirs.swap(0, 1);
                }
            }
            Action::RotLeft(num) => if !keep_front {
                self.set_current_dir_by_index(num, "pushd")?;
                self.rotate_left(num);
            },
            Action::RotRight(num) => if !keep_front {
                self.set_current_dir_by_index(len - (num % len), "pushd")?;
                self.rotate_right(num);
            },
            Action::Push(dir) => {
                let index = if keep_front { 1 } else { 0 };
                self.insert_dir(index, dir, variables);
            }
        };

        self.print_dirs();

        Ok(())
    }

    pub(crate) fn cd<I: IntoIterator>(
        &mut self,
        args: I,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>>
    where
        I::Item: AsRef<str>,
    {
        match args.into_iter().nth(1) {
            Some(dir) => {
                let dir = dir.as_ref();
                if dir == "-" {
                    self.switch_to_previous_directory(variables)
                } else {
                    self.change_and_push_dir(dir, variables)
                }
            }
            None => self.switch_to_home_directory(variables),
        }
    }

    fn switch_to_home_directory(&mut self, variables: &Variables) -> Result<(), Cow<'static, str>> {
        home_dir().map_or(
            Err(Cow::Borrowed("ion: failed to get home directory")),
            |home| {
                home.to_str().map_or(
                    Err(Cow::Borrowed(
                        "ion: failed to convert home directory to str",
                    )),
                    |home| self.change_and_push_dir(home, variables),
                )
            },
        )
    }

    fn switch_to_previous_directory(
        &mut self,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>> {
        self.get_previous_dir().cloned().map_or_else(
            || Err(Cow::Borrowed("ion: no previous directory to switch to")),
            |prev| {
                self.dirs.remove(1);
                let prev = prev.to_string_lossy().to_string();
                println!("{}", prev);
                self.change_and_push_dir(&prev, variables)
            },
        )
    }

    fn get_previous_dir(&self) -> Option<&PathBuf> {
        if self.dirs.len() < 2 {
            None
        } else {
            self.dirs.get(1)
        }
    }

    pub(crate) fn change_and_push_dir(
        &mut self,
        dir: &str,
        variables: &Variables,
    ) -> Result<(), Cow<'static, str>> {
        match (set_current_dir(dir), current_dir()) {
            (Ok(()), Ok(cur_dir)) => {
                self.push_dir(cur_dir, variables);
                Ok(())
            }
            (Err(err), _) => Err(Cow::Owned(format!(
                "ion: failed to set current dir to {}: {}\n",
                dir, err
            ))),
            (..) => Err(Cow::Borrowed(
                "ion: change_and_push_dir(): error occurred that should never happen\n",
            )), // This should not happen
        }
    }

    fn push_dir(&mut self, path: PathBuf, variables: &Variables) {
        self.dirs.push_front(path);

        self.dirs.truncate(DirectoryStack::get_size(variables));
    }

    fn insert_dir(&mut self, index: usize, path: PathBuf, variables: &Variables) {
        self.dirs.insert(index, path);
        self.dirs.truncate(DirectoryStack::get_size(variables));
    }

    pub(crate) fn dirs<I: IntoIterator>(&mut self, args: I) -> i32
    where
        I::Item: AsRef<str>,
    {
        const CLEAR: u8 = 1; // -c
        const ABS_PATHNAMES: u8 = 2; // -l
        const MULTILINE: u8 = 4; // -p | -v
        const INDEX: u8 = 8; // -v

        let mut dirs_args: u8 = 0;
        let mut num_arg: Option<usize> = None;

        for arg in args.into_iter().skip(1) {
            let arg = arg.as_ref();
            match arg {
                "-c" => dirs_args |= CLEAR,
                "-l" => dirs_args |= ABS_PATHNAMES,
                "-p" => dirs_args |= MULTILINE,
                "-v" => dirs_args |= INDEX | MULTILINE,
                arg => {
                    num_arg = match parse_numeric_arg(arg) {
                        Some((true, num)) => Some(num),
                        Some((false, num)) if self.dirs.len() > num => {
                            Some(self.dirs.len() - num - 1)
                        }
                        _ => return FAILURE, /* Err(Cow::Owned(format!("ion: dirs: {}: invalid
                                              * argument\n", arg))) */
                    };
                }
            }
        }

        if dirs_args & CLEAR > 0 {
            self.dirs.truncate(1);
        }

        let mapper: fn((usize, &PathBuf)) -> Cow<str> = match (
            dirs_args & ABS_PATHNAMES > 0,
            dirs_args & INDEX > 0,
        ) {
            // ABS, INDEX
            (true, true) => |(num, x)| Cow::Owned(format!(" {}  {}", num, try_abs_path(x))),
            (true, false) => |(_, x)| try_abs_path(x),
            (false, true) => |(num, x)| Cow::Owned(format!(" {}  {}", num, x.to_string_lossy())),
            (false, false) => |(_, x)| x.to_string_lossy(),
        };

        let mut iter = self.dirs.iter().enumerate().map(mapper);

        if let Some(num) = num_arg {
            match iter.nth(num) {
                Some(x) => println!("{}", x),
                None => return FAILURE,
            };
        } else {
            let folder: fn(String, Cow<str>) -> String = match dirs_args & MULTILINE > 0 {
                true => |x, y| x + "\n" + &y,
                false => |x, y| x + " " + &y,
            };

            let first = match iter.next() {
                Some(x) => x.to_string(),
                None => return SUCCESS,
            };

            println!("{}", iter.fold(first, folder));
        }
        SUCCESS
    }

    pub(crate) fn dir_from_top(&self, num: usize) -> Option<&PathBuf> { self.dirs.get(num) }

    pub(crate) fn dir_from_bottom(&self, num: usize) -> Option<&PathBuf> {
        self.dirs.iter().rev().nth(num)
    }

    fn print_dirs(&self) {
        let dir = self.dirs.iter().fold(String::new(), |acc, dir| {
            acc + " " + dir.to_str().unwrap_or("ion: no directory found")
        });
        println!("{}", dir.trim_left());
    }

    // sets current_dir to the element referred by index
    fn set_current_dir_by_index(
        &self,
        index: usize,
        caller: &str,
    ) -> Result<(), Cow<'static, str>> {
        let dir = self.dirs.iter().nth(index).ok_or_else(|| {
            Cow::Owned(format!(
                "ion: {}: {}: directory stack out of range\n",
                caller, index
            ))
        })?;

        set_current_dir(dir)
            .map_err(|_| Cow::Owned(format!("ion: {}: Failed setting current dir\n", caller)))
    }

    // pushd +<num>
    fn rotate_left(&mut self, num: usize) {
        let cloned = self.dirs.clone();
        for (dest, src) in self.dirs.iter_mut().zip(cloned.iter().cycle().skip(num)) {
            *dest = src.clone();
        }
    }

    // pushd -<num>
    fn rotate_right(&mut self, num: usize) {
        let len = self.dirs.len();
        self.rotate_left(len - (num % len));
    }
}

// parses -N or +N patterns
// required for popd, pushd, dirs
fn parse_numeric_arg(arg: &str) -> Option<(bool, usize)> {
    match arg.chars().nth(0) {
        Some('+') => Some(true),
        Some('-') => Some(false),
        _ => None,
    }.and_then(|b| arg[1..].parse::<usize>().ok().map(|num| (b, num)))
}

// converts pbuf to an absolute path if possible
fn try_abs_path(pbuf: &PathBuf) -> Cow<str> {
    Cow::Owned(
        pbuf.canonicalize()
            .unwrap_or_else(|_| pbuf.clone())
            .to_string_lossy()
            .to_string(),
    )
}
