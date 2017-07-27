use fnv::FnvHashMap;
use std::env;
use std::process;

use super::directory_stack::DirectoryStack;
use super::status::{FAILURE, SUCCESS};
use app_dirs::{AppDataType, AppInfo, app_root};
use liner::Context;
use types::{Array, ArrayVariableContext, HashMap, HashMapVariableContext, Identifier, Key, Value, VariableContext};

#[cfg(target_os = "redox")]
use sys::getpid;

#[cfg(all(unix, not(target_os = "unix")))]
use sys::getpid;

use sys::variables as self_sys;

#[derive(Debug)]
pub struct Variables {
    pub hashmaps: HashMapVariableContext,
    pub arrays: ArrayVariableContext,
    pub variables: VariableContext,
    pub aliases: VariableContext,
}

impl Default for Variables {
    fn default() -> Variables {
        let mut map = FnvHashMap::with_capacity_and_hasher(64, Default::default());
        map.insert("DIRECTORY_STACK_SIZE".into(), "1000".into());
        map.insert("HISTORY_SIZE".into(), "1000".into());
        map.insert("HISTORY_FILE_SIZE".into(), "1000".into());
        map.insert("PROMPT".into(), "\x1B\']\'0;${USER}: ${PWD}\x07\x1B\'[\'0m\x1B\'[\'1;38;5;85m${USER}\x1B\'[\'37m:\x1B\'[\'38;5;75m${PWD}\x1B\'[\'37m#\x1B\'[\'0m ".into());
        // Set the PID variable to the PID of the shell
        let pid = getpid().map(|p| p.to_string()).unwrap_or_else(
            |e| e.to_string(),
        );
        map.insert("PID".into(), pid.into());

        // Initialize the HISTORY_FILE variable
        if let Ok(mut home_path) =
            app_root(
                AppDataType::UserData,
                &AppInfo {
                    name: "ion",
                    author: "Redox OS Developers",
                },
            )
        {
            home_path.push("history");
            map.insert("HISTORY_FILE".into(), home_path.to_str().unwrap_or("?").into());
            map.insert("HISTORY_FILE_ENABLED".into(), "1".into());
        }

        // Initialize the PWD (Present Working Directory) variable
        env::current_dir().ok().map_or_else(
            || env::set_var("PWD", "?"),
            |path| {
                env::set_var("PWD", path.to_str().unwrap_or("?"))
            },
        );

        // Initialize the HOME variable
        env::home_dir().map_or_else(
            || env::set_var("HOME", "?"),
            |path| {
                env::set_var("HOME", path.to_str().unwrap_or("?"))
            },
        );
        Variables {
            hashmaps: FnvHashMap::with_capacity_and_hasher(64, Default::default()),
            arrays: FnvHashMap::with_capacity_and_hasher(64, Default::default()),
            variables: map,
            aliases: FnvHashMap::with_capacity_and_hasher(64, Default::default()),
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
                self.variables.insert(name.into(), value.into());
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

    pub fn set_hashmap_value(&mut self, name: &str, key: &str, value: &str) {
        if !name.is_empty() {
            if let Some(map) = self.hashmaps.get_mut(name) {
                map.insert(key.into(), value.into());
                return;
            }

            let mut map = HashMap::with_capacity_and_hasher(4, Default::default());
            map.insert(key.into(), value.into());
            self.hashmaps.insert(name.into(), map);
        }
    }

    pub fn get_map(&self, name: &str) -> Option<&HashMap> { self.hashmaps.get(name) }

    pub fn get_array(&self, name: &str) -> Option<&Array> { self.arrays.get(name) }

    pub fn unset_array(&mut self, name: &str) -> Option<Array> { self.arrays.remove(name) }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        self.variables.get(name).cloned().or_else(|| {
            env::var(name).map(Into::into).ok()
        })
    }

    pub fn get_var_or_empty(&self, name: &str) -> Value { self.get_var(name).unwrap_or_default() }

    pub fn unset_var(&mut self, name: &str) -> Option<Value> { self.variables.remove(name) }

    pub fn get_vars(&self) -> Vec<Identifier> {
        self.variables
            .keys()
            .cloned()
            .chain(env::vars().map(|(k, _)| k.into()))
            .collect()
    }

    pub fn is_valid_variable_character(c: char) -> bool { c.is_alphanumeric() || c == '_' || c == '?' }

    pub fn is_valid_variable_name(name: &str) -> bool { name.chars().all(Variables::is_valid_variable_character) }

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
                        let res = if neg { dir_stack.dir_from_top(num) } else { dir_stack.dir_from_bottom(num) };

                        if let Some(path) = res {
                            return Some(path.to_str().unwrap().to_string());
                        }
                    }
                    Err(_) => {
                        if let Some(home) = self_sys::get_user_home(tilde_prefix) {
                            return Some(home + remainder);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn command_expansion(&self, command: &str) -> Option<Value> {
        if let Ok(exe) = env::current_exe() {
            if let Ok(output) = process::Command::new(exe).arg("-c").arg(command).output() {
                if let Ok(mut stdout) = String::from_utf8(output.stdout) {
                    if stdout.ends_with('\n') {
                        stdout.pop();
                    }

                    return Some(stdout.into());
                }
            }
        }

        None
    }

    pub fn is_hashmap_reference(key: &str) -> Option<(Identifier, Key)> {
        let mut key_iter = key.split('[');

        if let Some(map_name) = key_iter.next() {
            if Variables::is_valid_variable_name(map_name) {
                if let Some(mut inner_key) = key_iter.next() {
                    if inner_key.ends_with(']') {
                        inner_key = inner_key.split(']').next().unwrap_or("");
                        inner_key = inner_key.trim_matches(|c| c == '\'' || c == '\"');
                        return Some((map_name.into(), inner_key.into()));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::{ExpanderFunctions, Select, expand_string};
    use shell::directory_stack::DirectoryStack;

    fn new_dir_stack() -> DirectoryStack { DirectoryStack::new() }

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
        let expanded = expand_string("$FOO", &get_expanders!(&variables, &new_dir_stack()), false).join("");
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn decompose_map_reference() {
        if let Some((map_name, inner_key)) = Variables::is_hashmap_reference("map[\'key\']") {
            assert!(map_name == "map".into());
            assert!(inner_key == "key".into());
        } else {
            assert!(false);
        }
    }
}
