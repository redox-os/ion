use super::{
    fork::Capture,
    sys::{self, env as sys_env, variables as self_sys},
    variables::Value,
    IonError, Shell,
};
use crate::{
    expansion::{self, Expander, ExpansionError, Select},
    types,
};
use std::{env, io::Read, iter::FromIterator, process};

impl<'a, 'b> Expander for Shell<'b> {
    type Error = IonError;

    /// Uses a subshell to expand a given command.
    fn command(&self, command: &str) -> expansion::Result<types::Str, Self::Error> {
        let output = self
            .fork(Capture::StdoutThenIgnoreStderr, move |shell| shell.on_command(command))
            .and_then(|result| {
                let mut string = String::with_capacity(1024);
                match result.stdout.unwrap().read_to_string(&mut string) {
                    Ok(_) => Ok(string),
                    Err(why) => Err(IonError::CaptureFailed(why)),
                }
            });

        // Ensure that the parent retains ownership of the terminal before exiting.
        let _ = sys::tcsetpgrp(libc::STDIN_FILENO, process::id());
        output.map(Into::into).map_err(|err| ExpansionError::Subprocess(Box::new(err)))
    }

    /// Expand a string variable given if its quoted / unquoted
    fn string(&self, name: &str) -> expansion::Result<types::Str, Self::Error> {
        if name == "?" {
            Ok(self.previous_status.into())
        } else {
            self.variables().get_str(name).map_err(Into::into)
        }
    }

    /// Expand an array variable with some selection
    fn array(
        &self,
        name: &str,
        selection: &Select<types::Str>,
    ) -> expansion::Result<types::Args, Self::Error> {
        match self.variables.get(name) {
            Some(Value::Array(array)) => match selection {
                Select::All => {
                    Ok(types::Args::from_iter(array.iter().map(|x| format!("{}", x).into())))
                }
                Select::Index(ref id) => id
                    .resolve(array.len())
                    .and_then(|n| array.get(n))
                    .map(|x| args![types::Str::from(format!("{}", x))])
                    .ok_or(ExpansionError::OutOfBound),
                Select::Range(ref range) => range
                    .bounds(array.len())
                    .and_then(|(start, length)| {
                        if array.len() > start {
                            Some(
                                array
                                    .iter()
                                    .skip(start)
                                    .take(length)
                                    .map(|var| format!("{}", var).into())
                                    .collect(),
                            )
                        } else {
                            None
                        }
                    })
                    .ok_or(ExpansionError::OutOfBound),
                Select::Key(_) => {
                    Err(ExpansionError::InvalidIndex(selection.clone(), "array", name.into()))
                }
            },
            Some(Value::HashMap(hmap)) => match selection {
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
                    Ok(array)
                }
                Select::Key(key) => {
                    Ok(args![format!("{}", hmap.get(&*key).unwrap_or(&Value::Str("".into())))])
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    Ok(args![format!(
                        "{}",
                        hmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => *n as isize,
                                Index::Backward(n) => -((*n + 1) as isize),
                            }
                            .to_string()
                        ))
                        .unwrap_or(&Value::Str("".into()))
                    )])
                }
                Select::Range(_) => {
                    Err(ExpansionError::InvalidIndex(selection.clone(), "hashmap", name.into()))
                }
            },
            Some(Value::BTreeMap(bmap)) => match selection {
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
                    Ok(array)
                }
                Select::Key(key) => {
                    Ok(args![format!("{}", bmap.get(&*key).unwrap_or(&Value::Str("".into())))])
                }
                Select::Index(index) => {
                    use crate::ranges::Index;
                    Ok(args![format!(
                        "{}",
                        bmap.get(&types::Str::from(
                            match index {
                                Index::Forward(n) => *n as isize,
                                Index::Backward(n) => -((*n + 1) as isize),
                            }
                            .to_string()
                        ))
                        .unwrap_or(&Value::Str("".into()))
                    )])
                }
                Select::Range(_) => {
                    Err(ExpansionError::InvalidIndex(selection.clone(), "btreemap", name.into()))
                }
            },
            None => Err(ExpansionError::VarNotFound),
            _ => Err(ExpansionError::ScalarAsArray(name.into())),
        }
    }

    fn map_keys(
        &self,
        name: &str,
        sel: &Select<types::Str>,
    ) -> expansion::Result<types::Args, Self::Error> {
        match self.variables.get(name) {
            Some(&Value::HashMap(ref map)) => {
                Self::select(map.keys().map(|x| format!("{}", x).into()), sel, map.len())
                    .ok_or(ExpansionError::InvalidIndex(sel.clone(), "map-like", name.into()))
            }
            Some(&Value::BTreeMap(ref map)) => {
                Self::select(map.keys().map(|x| format!("{}", x).into()), sel, map.len())
                    .ok_or(ExpansionError::InvalidIndex(sel.clone(), "map-like", name.into()))
            }
            Some(_) => Err(ExpansionError::NotAMap(name.into())),
            None => Err(ExpansionError::VarNotFound),
        }
    }

    fn map_values(
        &self,
        name: &str,
        sel: &Select<types::Str>,
    ) -> expansion::Result<types::Args, Self::Error> {
        match self.variables.get(name) {
            Some(&Value::HashMap(ref map)) => {
                Self::select(map.values().map(|x| format!("{}", x).into()), sel, map.len())
                    .ok_or(ExpansionError::InvalidIndex(sel.clone(), "map-like", name.into()))
            }
            Some(&Value::BTreeMap(ref map)) => {
                Self::select(map.values().map(|x| format!("{}", x).into()), sel, map.len())
                    .ok_or(ExpansionError::InvalidIndex(sel.clone(), "map-like", name.into()))
            }
            Some(_) => Err(ExpansionError::NotAMap(name.into())),
            None => Err(ExpansionError::VarNotFound),
        }
    }

    fn tilde(&self, input: &str) -> expansion::Result<types::Str, Self::Error> {
        // Only if the first character is a tilde character will we perform expansions
        if !input.starts_with('~') {
            return Ok(input.into());
        }

        let separator = input[1..].find(|c| c == '/' || c == '$');
        let (tilde_prefix, rest) = input[1..].split_at(separator.unwrap_or(input.len() - 1));

        match tilde_prefix {
            "" => sys_env::home_dir()
                .map(|home| types::Str::from(home.to_string_lossy().as_ref()) + rest)
                .ok_or(ExpansionError::HomeNotFound),
            "+" => Ok((env::var("PWD").unwrap_or_else(|_| "?".to_string()) + rest).into()),
            "-" => Ok((self.variables.get_str("OLDPWD")?.to_string() + rest).into()),
            _ => {
                let (neg, tilde_num) = if tilde_prefix.starts_with('+') {
                    (false, &tilde_prefix[1..])
                } else if tilde_prefix.starts_with('-') {
                    (true, &tilde_prefix[1..])
                } else {
                    (false, tilde_prefix)
                };

                match tilde_num.parse() {
                    Ok(num) => if neg {
                        self.directory_stack.dir_from_top(num)
                    } else {
                        self.directory_stack.dir_from_bottom(num)
                    }
                    .map(|path| path.to_str().unwrap().into())
                    .ok_or_else(|| ExpansionError::OutOfStack(num)),
                    Err(_) => self_sys::get_user_home(tilde_prefix)
                        .map(|home| (home + rest).into())
                        .ok_or(ExpansionError::HomeNotFound),
                }
            }
        }
    }
}
