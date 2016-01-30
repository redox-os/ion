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

    pub fn read(&mut self, args: &[String]) {
        let mut out = stdout();
        for arg in args.iter().skip(1) {
            print!("{}=", arg.trim());
            if let Err(message) = out.flush() {
                println!("{}: Failed to flush stdout", message);
            }
            if let Some(value) = readln() {
                self.set_var(arg, value.trim());
            }
        }
    }

    pub fn let_(&mut self, args: &[String]) {
        match (args.get(1), args.get(2)) {
            (Some(key), Some(value)) => {
                self.variables.insert(key.clone(), value.clone());
            }
            (Some(key), None) => {
                self.variables.remove(key);
            }
            _ => {
                for (key, value) in self.variables.iter() {
                    println!("{}={}", key, value);
                }
            }
        }
    }

    pub fn expand_variables(&self, jobs: &mut [Job]) {
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

    fn set_var(&mut self, name: &str, value: &str) {
        if !name.is_empty() {
            if value.is_empty() {
                self.variables.remove(&name.to_string());
            } else {
                self.variables.insert(name.to_string(), value.to_string());
            }
        }
    }
}
