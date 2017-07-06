use fnv::FnvHashMap;
use std::env;
use std::process;

use app_dirs::{AppDataType, AppInfo, app_root};
use super::directory_stack::DirectoryStack;
use liner::Context;
use super::status::{SUCCESS, FAILURE};
use types::{
    ArrayVariableContext,
    VariableContext,
    Identifier,
    Value,
    Array,
};

#[derive(Debug)]
pub struct Variables {
    pub arrays:    ArrayVariableContext,
    pub variables: VariableContext,
    pub aliases:   VariableContext,
}

impl Default for Variables {
    fn default() -> Variables {
        let mut map = FnvHashMap::with_capacity_and_hasher(
            64,
            Default::default(),
        );
        map.insert("DIRECTORY_STACK_SIZE".into(), "1000".into());
        map.insert("HISTORY_SIZE".into(), "1000".into());
        map.insert("HISTORY_FILE_ENABLED".into(), "1".into());
        map.insert("HISTORY_FILE_SIZE".into(), "1000".into());
        map.insert("PROMPT".into(), "\x1B\']\'0;${USER}: ${PWD}\x07\x1B\'[\'0m\x1B\'[\'1;38;5;85m${USER}\x1B\'[\'37m:\x1B\'[\'38;5;75m${PWD}\x1B\'[\'37m#\x1B\'[\'0m ".into());

        // Initialize the HISTORY_FILE variable
        if let Ok(mut home_path) = app_root(AppDataType::UserData, &AppInfo{ name: "ion", author: "Redox OS Developers" }) {
            home_path.push("ion_history");
            map.insert("HISTORY_FILE".into(), home_path.to_str().unwrap_or("?").into());
        }

        // Initialize the PWD (Present Working Directory) variable
        env::current_dir().ok().map_or_else(|| env::set_var("PWD", "?"), |path| env::set_var("PWD", path.to_str().unwrap_or("?")));

        // Initialize the HOME variable
        env::home_dir().map_or_else(|| env::set_var("HOME", "?"), |path| env::set_var("HOME", path.to_str().unwrap_or("?")));
        Variables {
            arrays: FnvHashMap::with_capacity_and_hasher(
                64,
                Default::default(),
            ),
            variables: map,
            aliases: FnvHashMap::with_capacity_and_hasher(
                64,
                Default::default(),
            )
        }
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

    pub fn set_var(&mut self, name: &str, value: &str) {
        if !name.is_empty() {
            if value.is_empty() {
                self.variables.remove(name);
            } else {
                self.variables.insert(
                    name.into(),
                    value.into(),
                );
            }
        }
    }

    pub fn set_array(&mut self, name: &str, value: Array) {
        if !name.is_empty() {
            if value.is_empty() {
                self.arrays.remove(name);
            } else {
                self.arrays.insert(name.into(), value);
            }
        }
    }

    pub fn get_array(&self, name: &str) -> Option<&Array> {
        self.arrays.get(name)
    }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        self.variables.get(name).cloned()
            .or_else(|| env::var(name).map(Into::into).ok())
    }

    pub fn get_var_or_empty(&self, name: &str) -> Value {
        self.get_var(name).unwrap_or_default()
    }

    pub fn unset_var(&mut self, name: &str) -> Option<Value> {
        self.variables.remove(name)
    }

    pub fn get_vars(&self) -> Vec<Identifier> {
        self.variables.keys().cloned()
            .chain(env::vars().map(|(k, _)| k.into()))
            .collect()
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

                match tilde_num.parse() {
                    Ok(num) => {
                        let res = if neg {
                            dir_stack.dir_from_top(num)
                        } else {
                            dir_stack.dir_from_bottom(num)
                        };

                        if let Some(path) = res {
                            return Some(path.to_str().unwrap().to_string());
                        }
                    }
                    Err(_) => {
                        if let Some(home) = get_user_home(tilde_prefix) {
                            return Some(home + remainder);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn command_expansion(&self, command: &str, quoted: bool) -> Option<Value> {
        use ascii_helpers::AsciiReplace;

        if let Ok(exe) = env::current_exe() {
            if let Ok(output) = process::Command::new(exe).arg("-c").arg(command).output() {
                if let Ok(mut stdout) = String::from_utf8(output.stdout) {
                    if stdout.ends_with('\n') {
                        stdout.pop();
                    }

                    return if quoted {
                        Some(stdout.into())
                    } else {
                        Some(stdout.ascii_replace('\n', ' ').into())
                    };
                }
            }
        }

        None
    }
}

#[cfg(all(unix, not(target_os = "redox")))]
fn get_user_home(username: &str) -> Option<String> {
    use users_unix::get_user_by_name;
    use users_unix::os::unix::UserExt;

    match get_user_by_name(username) {
        Some(user) => Some(user.home_dir().to_string_lossy().into_owned()),
        None => None,
    }
}

#[cfg(target_os = "redox")]
fn get_user_home(_username: &str) -> Option<String> {
    // TODO
    None
}

#[cfg(not(any(unix, target_os = "redox")))]
fn get_user_home(_username: &str) -> Option<String> {
    // TODO
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use shell::directory_stack::DirectoryStack;
    use parser::{expand_string, ExpanderFunctions, Select};

    fn new_dir_stack() -> DirectoryStack {
        DirectoryStack::new().unwrap()
    }

    #[test]
    fn undefined_variable_expands_to_empty_string() {

        let variables = Variables::default();
        let expanded = expand_string("$FOO", &get_expanders!(&variables, &new_dir_stack()), false).join("");
        assert_eq!("", &expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let expanded = expand_string(
            "$FOO",
            &get_expanders!(&variables, &new_dir_stack()),
            false
        ).join("");
        assert_eq!("BAR", &expanded);
    }
}
