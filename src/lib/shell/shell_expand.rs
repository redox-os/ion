use super::{escape::tilde, fork::Capture, variables::Value, Shell};
use crate::{
    parser::{Expander, Select},
    sys, types,
};
use std::{io::Read, iter::FromIterator, process};

impl<'a, 'b> Expander for Shell<'b> {
    /// Uses a subshell to expand a given command.
    fn command(&self, command: &str) -> Option<types::Str> {
        let mut output = None;
        match self.fork(Capture::StdoutThenIgnoreStderr, move |shell| shell.on_command(command)) {
            Ok(result) => {
                let mut string = String::with_capacity(1024);
                match result.stdout.unwrap().read_to_string(&mut string) {
                    Ok(_) => output = Some(string),
                    Err(why) => {
                        eprintln!("ion: error reading stdout of child: {}", why);
                    }
                }
            }
            Err(why) => {
                eprintln!("ion: fork error: {}", why);
            }
        }

        // Ensure that the parent retains ownership of the terminal before exiting.
        let _ = sys::tcsetpgrp(sys::STDIN_FILENO, process::id());
        output.map(Into::into)
    }

    /// Expand a string variable given if its quoted / unquoted
    fn string(&self, name: &str) -> Option<types::Str> {
        if name == "?" {
            Some(types::Str::from(self.previous_status.to_string()))
        } else {
            self.get::<types::Str>(name)
        }
    }

    /// Expand an array variable with some selection
    fn array(&self, name: &str, selection: &Select) -> Option<types::Args> {
        if let Some(array) = self.variables.get::<types::Array>(name) {
            match selection {
                Select::All => {
                    return Some(types::Args::from_iter(
                        array.iter().map(|x| format!("{}", x).into()),
                    ))
                }
                Select::Index(ref id) => {
                    return id
                        .resolve(array.len())
                        .and_then(|n| array.get(n))
                        .map(|x| types::Args::from_iter(Some(format!("{}", x).into())));
                }
                Select::Range(ref range) => {
                    if let Some((start, length)) = range.bounds(array.len()) {
                        if array.len() > start {
                            return Some(
                                array
                                    .iter()
                                    .skip(start)
                                    .take(length)
                                    .map(|var| format!("{}", var).into())
                                    .collect(),
                            );
                        }
                    }
                }
                _ => (),
            }
        } else if let Some(hmap) = self.variables.get::<types::HashMap>(name) {
            match selection {
                Select::All => {
                    let mut array = types::Args::new();
                    for (key, value) in hmap.iter() {
                        array.push(key.clone());
                        let f = format!("{}", value);
                        match *value {
                            Value::Str(_) => array.push(f.into()),
                            Value::Array(_) | Value::HashMap(_) | Value::BTreeMap(_) => {
                                for split in f.split_whitespace() {
                                    array.push(split.into());
                                }
                            }
                            _ => (),
                        }
                    }
                    return Some(array);
                }
                Select::Key(key) => {
                    return Some(args![format!(
                        "{}",
                        hmap.get(&*key).unwrap_or(&Value::Str("".into()))
                    )]);
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    return Some(args![format!(
                        "{}",
                        hmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => *n as isize,
                                Index::Backward(n) => -((*n + 1) as isize),
                            }
                            .to_string()
                        ))
                        .unwrap_or(&Value::Str("".into()))
                    )]);
                }
                _ => (),
            }
        } else if let Some(bmap) = self.variables.get::<types::BTreeMap>(name) {
            match selection {
                Select::All => {
                    let mut array = types::Args::new();
                    for (key, value) in bmap.iter() {
                        array.push(key.clone());
                        let f = format!("{}", value);
                        match *value {
                            Value::Str(_) => array.push(f.into()),
                            Value::Array(_) | Value::HashMap(_) | Value::BTreeMap(_) => {
                                for split in f.split_whitespace() {
                                    array.push(split.into());
                                }
                            }
                            _ => (),
                        }
                    }
                    return Some(array);
                }
                Select::Key(key) => {
                    return Some(args![format!(
                        "{}",
                        bmap.get(&*key).unwrap_or(&Value::Str("".into()))
                    )]);
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    return Some(args![format!(
                        "{}",
                        bmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => *n as isize,
                                Index::Backward(n) => -((*n + 1) as isize),
                            }
                            .to_string()
                        ))
                        .unwrap_or(&Value::Str("".into()))
                    )]);
                }
                _ => (),
            }
        }
        None
    }

    fn map_keys(&self, name: &str, sel: &Select) -> Option<types::Args> {
        match self.variables.get_ref(name) {
            Some(&Value::HashMap(ref map)) => {
                Self::select(map.keys().map(|x| format!("{}", x).into()), sel, map.len())
            }
            Some(&Value::BTreeMap(ref map)) => {
                Self::select(map.keys().map(|x| format!("{}", x).into()), sel, map.len())
            }
            _ => None,
        }
    }

    fn map_values(&self, name: &str, sel: &Select) -> Option<types::Args> {
        match self.variables.get_ref(name) {
            Some(&Value::HashMap(ref map)) => {
                Self::select(map.values().map(|x| format!("{}", x).into()), sel, map.len())
            }
            Some(&Value::BTreeMap(ref map)) => {
                Self::select(map.values().map(|x| format!("{}", x).into()), sel, map.len())
            }
            _ => None,
        }
    }

    fn tilde(&self, input: &str) -> Option<String> {
        tilde(
            input,
            &self.directory_stack,
            self.variables.get::<types::Str>("OLDPWD").as_ref().map(types::Str::as_str),
        )
    }
}
