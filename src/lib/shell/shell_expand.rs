use super::{
    pipe_exec::create_pipe, sys::NULL_PATH, variables::Value, IonError, PipelineError, Shell,
};
use crate::{
    expansion::{Error, Expander, Result, Select},
    types,
};
use nix::unistd::{tcsetpgrp, Pid};
#[cfg(target_os = "redox")]
use redox_users::All;
use std::{env, fs::File, io::Read};
#[cfg(not(target_os = "redox"))]
use users::os::unix::UserExt;

impl<'a, 'b> Expander for Shell<'b> {
    type Error = IonError;

    /// Uses a subshell to expand a given command.
    fn command(
        &mut self,
        command: &str,
        set_cmd_duration: bool,
    ) -> Result<types::Str, Self::Error> {
        let (mut reader, writer) = create_pipe()
            .map_err(|err| Error::Subprocess(Box::new(IonError::PipelineExecutionError(err))))?;
        let null_file = File::open(NULL_PATH).map_err(|err| {
            Error::Subprocess(Box::new(IonError::PipelineExecutionError(
                PipelineError::CaptureFailed(err),
            )))
        })?;

        // Store the previous default redirections
        let prev_stdout = self.stdout(writer);
        let prev_stderr = self.stderr(null_file);

        // Execute the command
        let result = self
            .on_command(command, set_cmd_duration)
            .map_err(|err| Error::Subprocess(Box::new(err)));

        // Reset the pipes, droping the stdout
        self.stdout(prev_stdout);
        self.stderr(prev_stderr);

        // Ensure that the parent retains ownership of the terminal before exiting.
        let _ = tcsetpgrp(nix::libc::STDIN_FILENO, Pid::this());

        result?;

        let mut string = String::with_capacity(1024);
        match reader.read_to_string(&mut string) {
            Ok(_) => Ok(string.into()),
            Err(why) => Err(Error::Subprocess(Box::new(PipelineError::CaptureFailed(why).into()))),
        }
    }

    /// Expand a string variable given if its quoted / unquoted
    fn string(&self, name: &str) -> Result<types::Str, Self::Error> {
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
    ) -> Result<types::Args, Self::Error> {
        match self.variables.get(name) {
            Some(Value::Array(array)) => match selection {
                Select::All => Ok(array.iter().map(|x| format!("{}", x).into()).collect()),
                Select::Index(ref id) => id
                    .resolve(array.len())
                    .and_then(|n| array.get(n))
                    .map(|x| args![types::Str::from(format!("{}", x))])
                    .ok_or(Error::OutOfBound { length: array.len(), index: *id }),
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
                    .ok_or(Error::InvalidRange { length: array.len(), range: *range }),
                Select::Key(_) => Err(Error::InvalidIndex(selection.clone(), "array", name.into())),
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
                    Err(Error::InvalidIndex(selection.clone(), "hashmap", name.into()))
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
                    Err(Error::InvalidIndex(selection.clone(), "btreemap", name.into()))
                }
            },
            None => Err(Error::VarNotFound),
            _ => Err(Error::ScalarAsArray(name.into())),
        }
    }

    fn map_keys(&self, name: &str) -> Result<types::Args, Self::Error> {
        match self.variables.get(name) {
            Some(&Value::HashMap(ref map)) => {
                Ok(map.keys().map(|x| x.to_string().into()).collect())
            }
            Some(&Value::BTreeMap(ref map)) => {
                Ok(map.keys().map(|x| x.to_string().into()).collect())
            }
            Some(_) => Err(Error::NotAMap(name.into())),
            None => Err(Error::VarNotFound),
        }
    }

    fn map_values(&self, name: &str) -> Result<types::Args, Self::Error> {
        match self.variables.get(name) {
            Some(&Value::HashMap(ref map)) => {
                Ok(map.values().map(|x| x.to_string().into()).collect())
            }
            Some(&Value::BTreeMap(ref map)) => {
                Ok(map.values().map(|x| x.to_string().into()).collect())
            }
            Some(_) => Err(Error::NotAMap(name.into())),
            None => Err(Error::VarNotFound),
        }
    }

    fn tilde(&self, input: &str) -> Result<types::Str, Self::Error> {
        // Only if the first character is a tilde character will we perform expansions
        if !input.starts_with('~') {
            return Ok(input.into());
        }

        let separator = input[1..].find(|c| c == '/' || c == '$');
        let (tilde_prefix, rest) = input[1..].split_at(separator.unwrap_or(input.len() - 1));

        match tilde_prefix {
            "+" => Ok(env::var("PWD").unwrap_or_else(|_| "?".into()).into()),
            "-" => Ok(self.variables.get_str("OLDPWD")?),
            _ => {
                let (neg, tilde_num) = if let Some(prefstripped) = tilde_prefix.strip_prefix('+') {
                    (false, prefstripped)
                } else if let Some(prefstripped) = tilde_prefix.strip_prefix('-') {
                    (true, prefstripped)
                } else {
                    (false, tilde_prefix)
                };

                if let Ok(num) = tilde_num.parse() {
                    if neg {
                        self.directory_stack.dir_from_top(num)
                    } else {
                        self.directory_stack.dir_from_bottom(num)
                    }
                    .map(|path| path.to_str().unwrap().into())
                    .ok_or(Error::OutOfStack(num))
                } else {
                    #[cfg(not(target_os = "redox"))]
                    {
                        let user = if tilde_prefix.is_empty() {
                            users::get_user_by_uid(users::get_current_uid())
                        } else {
                            users::get_user_by_name(tilde_prefix)
                        };
                        match user {
                            Some(user) => Ok(user.home_dir().to_string_lossy().as_ref().into()),
                            None => Err(Error::HomeNotFound),
                        }
                    }
                    #[cfg(target_os = "redox")]
                    {
                        if let Ok(users) =
                            redox_users::AllUsers::basic(redox_users::Config::default())
                        {
                            let user = if tilde_prefix.is_empty() {
                                redox_users::get_uid().ok().and_then(|id| users.get_by_id(id))
                            } else {
                                users.get_by_name(tilde_prefix)
                            };
                            match user {
                                Some(user) => Ok(user.home.as_str().into()),
                                None => Err(Error::HomeNotFound),
                            }
                        } else {
                            Err(Error::HomeNotFound)
                        }
                    }
                }
            }
        }
        .map(|home: types::Str| home + rest)
    }
}
