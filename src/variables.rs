use std::collections::BTreeMap;
use std::io::{stdout, Write};
use std::env;

use super::peg::{Pipeline, Job};
use super::input_editor::readln;
use super::status::{SUCCESS, FAILURE};
use super::directory_stack::DirectoryStack;

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
        let mut out = stdout();
        for arg in args.into_iter().skip(1) {
            print!("{}=", arg.as_ref().trim());
            if let Err(message) = out.flush() {
                println!("{}: Failed to flush stdout", message);
                return FAILURE;
            }
            if let Some(value) = readln() {
                self.set_var(arg.as_ref(), value.trim());
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
            if let None = self.unset_var(variable.as_ref()) {
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
        self.get_var(name).unwrap_or(String::new())
    }

    pub fn unset_var(&mut self, name: &str) -> Option<String> {
        self.variables.remove(name)
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

    pub fn expand_job(&self, job: &Job, dir_stack: &DirectoryStack) -> Job {
        // TODO don't copy everything
        Job::new(job.args
                     .iter()
                     .map(|original: &String| self.expand_string(&original, dir_stack))
                     .collect(),
                 job.background)
    }

    fn replace_substring(string: &mut String, start: usize, end: usize, replacement: &str) {
        let string_start = string.chars().take(start).collect::<String>();
        let string_end = string.chars().skip(end+1).collect::<String>();
        *string = string_start + replacement + &string_end;
    }

    pub fn is_valid_variable_character(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '?'
    }

    pub fn is_valid_variable_name(name: &str) -> bool {
        name.chars().all(Variables::is_valid_variable_character)
    }

    pub fn tilde_expansion(&self, word: String, dir_stack: &DirectoryStack) -> String {
        // If the word doesn't start with ~, just return it to avoid allocating an iterator
        if word.starts_with("~") {
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
                        return home.to_string_lossy().to_string() + remainder;
                    }
                }
                "+" => {
                    if let Some(pwd) = self.get_var("PWD") {
                        return pwd.to_string() + remainder;
                    } else if let Ok(pwd) = env::current_dir() {
                        return pwd.to_string_lossy().to_string() + remainder;
                    }
                }
                "-" => {
                    if let Some(oldpwd) = self.get_var("OLDPWD") {
                        return oldpwd.to_string() + remainder;
                    }
                }
                _ => {
                    let neg;
                    let tilde_num;

                    if tilde_prefix.starts_with("+") {
                        tilde_num = &tilde_prefix[1..];
                        neg = false;
                    } else if tilde_prefix.starts_with("-") {
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
                            return path.to_str().unwrap().to_string();
                        }
                    }
                }
            }
        }

        word
    }

    pub fn expand_string<'a>(&'a self, original: &'a str, dir_stack: &DirectoryStack) -> String {
        let mut new = original.to_owned();
        new = self.tilde_expansion(new, dir_stack);
        let mut replacements: Vec<(usize, usize, String)> = vec![];
        for (n, _) in original.match_indices('$') {
            if n > 0 {
                if let Some(c) = original.chars().nth(n-1) {
                    if c == '\\' {
                        continue;
                    }
                }
            }
            let mut var_name = "".to_owned();
            for (i, c) in original.char_indices().skip(n+1) { // skip the dollar sign
                if Variables::is_valid_variable_character(c) {
                    var_name.push(c);
                    if i == original.len() - 1 {
                        replacements.push((n, i, var_name.clone()));
                        break;
                    }
                } else {
                    replacements.push((n, i-1, var_name.clone()));
                    break;
                }
            }
        }

        for &(start, end, ref var_name) in replacements.iter().rev() {
            if let Some(value) = self.get_var(var_name) {
                Variables::replace_substring(&mut new, start, end, &value);
            } else {
                Variables::replace_substring(&mut new, start, end, "");
            }
        }
        new.clone()
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
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("", &expanded);
    }

    #[test]
    fn let_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn let_fails_if_no_value() {
        let mut variables = Variables::default();
        let return_status = variables.let_(vec!["let", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn expand_several_variables() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        variables.let_(vec!["let", "X", "=", "Y"]);
        let expanded = variables.expand_string("variables: $FOO $X", &new_dir_stack());
        assert_eq!("variables: BAR Y", &expanded);
    }

    #[test]
    fn replace_substring() {
        let mut string = "variable: $FOO".to_owned();
        Variables::replace_substring(&mut string, 10, 13, "BAR");
        assert_eq!("variable: BAR", string);
    }

    #[test]
    fn escape_with_backslash() {
        let variables = Variables::default();
        let expanded = variables.expand_string("\\$FOO", &new_dir_stack());
        assert_eq!("\\$FOO", &expanded);
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
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
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
