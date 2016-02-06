use std::collections::BTreeMap;
use std::io::{stdout, Write};

use super::peg::Job;
use super::input_editor::readln;

pub struct Variables {
    variables: BTreeMap<String, String>,
}

impl Variables {
    pub fn new() -> Variables {
        Variables { variables: BTreeMap::new() }
    }

    pub fn read<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        let mut out = stdout();
        for arg in args.into_iter().skip(1) {
            print!("{}=", arg.as_ref().trim());
            if let Err(message) = out.flush() {
                println!("{}: Failed to flush stdout", message);
            }
            if let Some(value) = readln() {
                self.set_var(arg.as_ref(), value.trim());
            }
        }
    }

    pub fn let_<I: IntoIterator>(&mut self, args: I)
        where I::Item: AsRef<str>
    {
        let mut args = args.into_iter();
        match (args.next(), args.next()) {
            (Some(key), Some(value)) => {
                self.variables.insert(key.as_ref().to_string(), value.as_ref().to_string());
            }
            (Some(key), None) => {
                self.variables.remove(key.as_ref());
            }
            _ => {
                for (key, value) in self.variables.iter() {
                    println!("{}={}", key, value);
                }
            }
        }
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

    pub fn expand_variables(&self, jobs: &mut [Job]) {
        // TODO don't copy everything
        for mut job in &mut jobs[..] {
            job.command = self.expand_string(&job.command).to_string();
            job.args = job.args
                          .iter()
                          .map(|original: &String| self.expand_string(&original).to_string())
                          .collect();
        }
    }

    #[inline]
    fn expand_string<'a>(&'a self, original: &'a str) -> &'a str {
        if original.starts_with("$") {
            if let Some(value) = self.variables.get(&original[1..]) {
                &value
            } else {
                ""
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
        variables.let_(vec!["FOO", "BAR"]);
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
        variables.let_(vec!["FOO", "BAR"]);
        variables.let_(vec!["FOO"]);
        let expanded = variables.expand_string("$FOO");
        assert_eq!("", expanded);
    }
}
