use std::collections::BTreeMap;
use std::env;
use std::iter;
use std::process;

use liner::Context;

use super::peg::{Pipeline, Job};
use super::status::{SUCCESS, FAILURE};
use super::directory_stack::DirectoryStack;
use super::shell_expand::{self, ExpandErr};

pub struct Variables {
    variables: BTreeMap<String, String>,
}

impl Default for Variables {
    fn default() -> Variables {
        Variables { variables: BTreeMap::new() }
    }
}

impl Variables {
    pub fn read<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        let mut con = Context::new();
        for arg in args.into_iter().skip(1) {
            match con.read_line(format!("{}=", arg.as_ref().trim()), &mut |_| {}) {
                Ok(buffer) => self.set_var(arg.as_ref(), buffer.trim()),
                Err(_) => return FAILURE,
            }
        }
        SUCCESS
    }

    pub fn let_<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        match Variables::parse_assignment(args) {
            (Some(key), Some(value)) => {
                if !Variables::is_valid_variable_name(&key) {
                    println!("Invalid variable name");
                    return FAILURE;
                }
                self.variables.insert(key.to_string(), value.to_string());
            },
            (Some(_), None) => {
                println!("Please provide a value for the variable");
                return FAILURE;
            },
            _ => {
                for (key, value) in &self.variables {
                    println!("{}={}", key, value);
                }
            }
        }
        SUCCESS
    }

    pub fn drop_variable<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        let args = args.into_iter().collect::<Vec<I::Item>>();
        if args.len() <= 1 {
            println!("You must specify a variable name");
            return FAILURE;
        }
        for variable in args.iter().skip(1) {
            if self.unset_var(variable.as_ref()).is_none() {
                println!("Undefined variable: {}", variable.as_ref());
                return FAILURE;
            }
        }
        SUCCESS
    }

    pub fn set_var(&mut self, name: &str, value: &str) {
        if !name.is_empty() {
            if value.is_empty() {
                self.variables.remove(&name.to_string());
            } else {
                self.variables.insert(name.to_string(), value.to_string());
            }
        }
    }

    pub fn get_var(&self, name: &str) -> Option<String> {
        self.variables.get(name).cloned().or(env::var(name).ok())
    }

    pub fn get_var_or_empty(&self, name: &str) -> String {
        self.get_var(name).unwrap_or_default()
    }

    pub fn unset_var(&mut self, name: &str) -> Option<String> {
        self.variables.remove(name)
    }

    pub fn get_vars(&self) -> Vec<String> {
        self.variables.keys().cloned().chain(env::vars().map(|(k, _)| k)).collect()
    }

    fn parse_assignment<I: IntoIterator>(args: I) -> (Option<String>, Option<String>)
        where I::Item: AsRef<str>
    {
        let args = args.into_iter();
        let string: String = args.skip(1).fold(String::new(), |string, x| string + x.as_ref());
        let mut split = string.split('=');
        (split.next().and_then(|x| if x == "" { None } else { Some(x.to_owned()) }), split.next().and_then(|x| Some(x.to_owned())))
    }

    pub fn export_variable<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        match Variables::parse_assignment(args) {
            (Some(key), Some(value)) => {
                if !Variables::is_valid_variable_name(&key) {
                    println!("Invalid variable name");
                    return FAILURE;
                }
                env::set_var(key, value);
            },
            (Some(key), None) => {
                if let Some(local_value) = self.get_var(&key) {
                    env::set_var(key, local_value);
                } else {
                    println!("Unknown variable: {}", &key);
                    return FAILURE;
                }
            },
            _ => {
                println!("Usage: export KEY=VALUE");
                return FAILURE;
            }
        }
        SUCCESS
    }

    pub fn expand_pipeline(&self, pipeline: &Pipeline, dir_stack: &DirectoryStack) -> Pipeline {
        // TODO don't copy everything
        // TODO ugh, I made it worse
        Pipeline::new(pipeline.jobs.iter().map(|job| self.expand_job(job, dir_stack)).collect(),
                      pipeline.stdin.clone(),
                      pipeline.stdout.clone())
    }

    /// Takes the current job's arguments and expands them, one argument at a
    /// time, returning a new `Job` with the expanded arguments.
    pub fn expand_job(&self, job: &Job, dir_stack: &DirectoryStack) -> Job {
        // Expand each of the current job's arguments using the `expand_string` method.
        // If an error occurs, mark that error and break;
        let mut expanded: Vec<String> = Vec::new();
        let mut nth_argument = 0;
        let mut error_occurred = None;
        for (job, result) in job.args.iter().map(|argument| self.expand_string(argument, dir_stack)).enumerate() {
            match result {
                Ok(expanded_string) => expanded.push(expanded_string),
                Err(cause) => {
                    nth_argument   = job;
                    error_occurred = Some(cause);
                    expanded = vec!["".to_owned()];
                    break
                }
            }
        }

        // If an error was detected, handle that error.
        if let Some(cause) = error_occurred {
            match cause {
                ExpandErr::UnmatchedBraces(position) => {
                    let original = job.args.join(" ");
                    let n_chars = job.args.iter().take(nth_argument)
                        .fold(0, |total, arg| total + 1 + arg.len()) + position;
                    println!("ion: expand error: unmatched braces");
                    println!("{}", original);
                    println!("{}^", iter::repeat("-").take(n_chars).collect::<String>());
                },
                ExpandErr::InnerBracesNotImplemented => {
                    println!("ion: expand error: inner braces not yet implemented");
                }
            }
        }

        Job::new(expanded, job.background)
    }

    pub fn is_valid_variable_character(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '?'
    }

    pub fn is_valid_variable_name(name: &str) -> bool {
        name.chars().all(Variables::is_valid_variable_character)
    }

    pub fn tilde_expansion(&self, word: &str, dir_stack: &DirectoryStack) -> Option<String> {
        let mut chars = word.char_indices();

        let tilde_prefix;
        let remainder;

        loop {
            if let Some((ind, c)) = chars.next() {
                if c == '/' || c == '$' {
                    tilde_prefix = &word[1..ind];
                    remainder = &word[ind..];
                    break;
                }
            } else {
                tilde_prefix = &word[1..];
                remainder = "";
                break;
            }
        }

        match tilde_prefix {
            "" => {
                if let Some(home) = env::home_dir() {
                    return Some(home.to_string_lossy().to_string() + remainder);
                }
            }
            "+" => {
                if let Some(pwd) = self.get_var("PWD") {
                    return Some(pwd.to_string() + remainder);
                } else if let Ok(pwd) = env::current_dir() {
                    return Some(pwd.to_string_lossy().to_string() + remainder);
                }
            }
            "-" => {
                if let Some(oldpwd) = self.get_var("OLDPWD") {
                    return Some(oldpwd.to_string() + remainder);
                }
            }
            _ => {
                let neg;
                let tilde_num;

                if tilde_prefix.starts_with('+') {
                    tilde_num = &tilde_prefix[1..];
                    neg = false;
                } else if tilde_prefix.starts_with('-') {
                    tilde_num = &tilde_prefix[1..];
                    neg = true;
                } else {
                    tilde_num = tilde_prefix;
                    neg = false;
                }

                if let Ok(num) = tilde_num.parse::<usize>() {
                    let res = if neg {
                        dir_stack.dir_from_top(num)
                    } else {
                        dir_stack.dir_from_bottom(num)
                    };

                    if let Some(path) = res {
                        return Some(path.to_str().unwrap().to_string());
                    }
                }
            }
        }
        None
    }

    pub fn command_expansion(&self, command: &str) -> Option<String> {
        if let Ok(exe) = env::current_exe() {
            if let Ok(output) = process::Command::new(exe).arg("-c").arg(command).output() {
                if let Ok(mut stdout) = String::from_utf8(output.stdout) {
                    if stdout.ends_with('\n') {
                        stdout.pop();
                    }
                    return Some(stdout);
                }
            }
        }

        None
    }

    /// Takes an argument string as input and expands it.
    pub fn expand_string<'a>(&'a self, original: &'a str, dir_stack: &DirectoryStack) -> Result<String, ExpandErr> {
        let tilde_fn    = |tilde:    &str| self.tilde_expansion(tilde, dir_stack);
        let variable_fn = |variable: &str| self.get_var(variable);
        let command_fn  = |command:  &str| self.command_expansion(command);
        shell_expand::expand_string(original, tilde_fn, variable_fn, command_fn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use status::{FAILURE, SUCCESS};
    use directory_stack::DirectoryStack;

    fn new_dir_stack() -> DirectoryStack {
        DirectoryStack::new().unwrap()
    }

    #[test]
    fn undefined_variable_expands_to_empty_string() {
        let variables = Variables::default();
        let expanded = variables.expand_string("$FOO", &new_dir_stack()).unwrap();
        assert_eq!("", &expanded);
    }

    #[test]
    fn let_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        let expanded = variables.expand_string("$FOO", &new_dir_stack()).unwrap();
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let expanded = variables.expand_string("$FOO", &new_dir_stack()).unwrap();
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn let_fails_if_no_value() {
        let mut variables = Variables::default();
        let return_status = variables.let_(vec!["let", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn let_checks_variable_name() {
        let mut variables = Variables::default();
        let return_status = variables.let_(vec!["let", ",;!:", "=", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_deletes_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let return_status = variables.drop_variable(vec!["drop", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = variables.expand_string("$FOO", &new_dir_stack()).unwrap();
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = variables.drop_variable(vec!["drop"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut variables = Variables::default();
        let return_status = variables.drop_variable(vec!["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }
}
