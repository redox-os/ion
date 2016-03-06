use std::collections::BTreeMap;
use std::io::{stdout, Write};

use super::peg::Job;
use super::input_editor::readln;
use super::status::{SUCCESS, FAILURE};

pub struct Variables {
    variables: BTreeMap<String, String>,
}

impl Variables {
    pub fn new() -> Variables {
        Variables { variables: BTreeMap::new() }
    }

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
        let args = args.into_iter();
        let string: String = args.skip(1).fold(String::new(), |string, x| string + x.as_ref());
        let mut split = string.split('=');
        match (split.next().and_then(|x| if x == "" { None } else { Some(x) }), split.next()) {
            (Some(key), Some(value)) => {
                self.variables.insert(key.to_string(), value.to_string());
            },
            (Some(key), None) => {
                self.variables.remove(key);
            },
            _ => {
                for (key, value) in self.variables.iter() {
                    println!("{}={}", key, value);
                }
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

    pub fn expand_job(&self, job: &Job) -> Job {
        // TODO don't copy everything
        Job::from_vec_string(job.args
                                .iter()
                                .map(|original: &String| self.expand_string(&original).to_string())
                                .collect(),
                             job.background)
    }

    #[inline]
    pub fn expand_string<'a>(&'a self, original: &'a str) -> &'a str {
        if original.starts_with("$") {
            match self.variables.get(&original[1..]) {
                Some(value) => &value,
                None        => ""
            }
        } else {
            original
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_variable_expands_to_empty_string() {
        let variables = Variables::new();
        let expanded = variables.expand_string("$FOO");
        assert_eq!("", expanded);
    }

    #[test]
    fn let_and_expand_a_variable() {
        let mut variables = Variables::new();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        let expanded = variables.expand_string("$FOO");
        assert_eq!("BAR", expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::new();
        variables.set_var("FOO", "BAR");
        let expanded = variables.expand_string("$FOO");
        assert_eq!("BAR", expanded);
    }

    #[test]
    fn remove_a_variable_with_let() {
        let mut variables = Variables::new();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        variables.let_(vec!["let", "FOO"]);
        let expanded = variables.expand_string("$FOO");
        assert_eq!("", expanded);
    }
}
