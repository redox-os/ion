use super::colors::Colors;
use super::directory_stack::DirectoryStack;
use super::plugins::namespaces::{self, StringNamespace};
use super::status::{FAILURE, SUCCESS};
use fnv::FnvHashMap;
use liner::Context;
use std::env;
use std::io::{self, BufRead};
use sys::{self, getpid, is_root};
use sys::variables as self_sys;
use types::{
    Array, ArrayVariableContext, HashMap, HashMapVariableContext, Identifier, Key, Value,
    VariableContext,
};
use unicode_segmentation::UnicodeSegmentation;
use xdg::BaseDirectories;

lazy_static! {
    static ref STRING_NAMESPACES: FnvHashMap<Identifier, StringNamespace> = namespaces::collect();
}

#[derive(Clone, Debug)]
pub struct Variables {
    pub hashmaps:  HashMapVariableContext,
    pub arrays:    ArrayVariableContext,
    pub variables: VariableContext,
    pub aliases:   VariableContext,
    flags:         u8,
}

impl Default for Variables {
    fn default() -> Variables {
        let mut map = FnvHashMap::with_capacity_and_hasher(64, Default::default());
        map.insert("DIRECTORY_STACK_SIZE".into(), "1000".into());
        map.insert("HISTORY_SIZE".into(), "1000".into());
        map.insert("HISTFILE_SIZE".into(), "1000".into());
        map.insert(
            "PROMPT".into(),
            "${x::1B}]0;${USER}: \
             ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# \
             ${c::reset}"
                .into(),
        );
        // Set the PID variable to the PID of the shell
        let pid = getpid()
            .map(|p| p.to_string())
            .unwrap_or_else(|e| e.to_string());
        map.insert("PID".into(), pid.into());

        // Initialize the HISTFILE variable
        if let Ok(base_dirs) = BaseDirectories::with_prefix("ion") {
            if let Ok(mut path) = base_dirs.place_data_file("history") {
                map.insert("HISTFILE".into(), path.to_str().unwrap_or("?").into());
                map.insert("HISTFILE_ENABLED".into(), "1".into());
            }
        }

        // Initialize the PWD (Present Working Directory) variable
        env::current_dir().ok().map_or_else(
            || env::set_var("PWD", "?"),
            |path| env::set_var("PWD", path.to_str().unwrap_or("?")),
        );

        // Initialize the HOME variable
        env::home_dir().map_or_else(
            || env::set_var("HOME", "?"),
            |path| env::set_var("HOME", path.to_str().unwrap_or("?")),
        );
        Variables {
            hashmaps:  FnvHashMap::with_capacity_and_hasher(64, Default::default()),
            arrays:    FnvHashMap::with_capacity_and_hasher(64, Default::default()),
            variables: map,
            aliases:   FnvHashMap::with_capacity_and_hasher(64, Default::default()),
            flags:     0,
        }
    }
}

const PLUGIN: u8 = 1;

impl Variables {
    pub(crate) fn has_plugin_support(&self) -> bool { self.flags & PLUGIN != 0 }

    pub(crate) fn enable_plugins(&mut self) { self.flags |= PLUGIN; }

    pub(crate) fn disable_plugins(&mut self) { self.flags &= 255 ^ PLUGIN; }

    pub(crate) fn read<I: IntoIterator>(&mut self, args: I) -> i32
    where
        I::Item: AsRef<str>,
    {
        if sys::isatty(sys::STDIN_FILENO) {
            let mut con = Context::new();
            for arg in args.into_iter().skip(1) {
                match con.read_line(format!("{}=", arg.as_ref().trim()), &mut |_| {}) {
                    Ok(buffer) => self.set_var(arg.as_ref(), buffer.trim()),
                    Err(_) => return FAILURE,
                }
            }
        } else {
            let stdin = io::stdin();
            let handle = stdin.lock();
            let mut lines = handle.lines();
            for arg in args.into_iter().skip(1) {
                if let Some(Ok(line)) = lines.next() {
                    self.set_var(arg.as_ref(), line.trim());
                }
            }
        }
        SUCCESS
    }

    pub fn set_var(&mut self, name: &str, value: &str) {
        if !name.is_empty() {
            if value.is_empty() {
                self.variables.remove(name);
            } else {
                if name == "NS_PLUGINS" {
                    match value {
                        "0" => self.disable_plugins(),
                        "1" => self.enable_plugins(),
                        _ => eprintln!(
                            "ion: unsupported value for NS_PLUGINS. Value must be either 0 or 1."
                        ),
                    }
                    return;
                }
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

    #[allow(dead_code)]
    pub(crate) fn set_hashmap_value(&mut self, name: &str, key: &str, value: &str) {
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

    /// Obtains the value for the **SWD** variable.
    ///
    /// Useful for getting smaller prompts, this will produce a simplified variant of the
    /// working directory which the leading `HOME` prefix replaced with a tilde character.
    fn get_simplified_directory(&self) -> Value {
        self.get_var("PWD")
            .unwrap()
            .replace(&self.get_var("HOME").unwrap(), "~")
    }

    /// Obtains the value for the **MWD** variable.
    ///
    /// Further minimizes the directory path in the same manner that Fish does by default.
    /// That is, if more than two parents are visible in the path, all parent directories
    /// of the current directory will be reduced to a single character.
    fn get_minimal_directory(&self) -> Value {
        let swd = self.get_simplified_directory();

        {
            // Temporarily borrow the `swd` variable while we attempt to assemble a minimal
            // variant of the directory path. If that is not possible, we will cancel the
            // borrow and return `swd` itself as the minified path.
            let elements = swd.split("/")
                .filter(|s| !s.is_empty())
                .collect::<Vec<&str>>();
            if elements.len() > 2 {
                let mut output = String::new();
                for element in &elements[0..elements.len() - 1] {
                    let mut segmenter = UnicodeSegmentation::graphemes(*element, true);
                    let grapheme = segmenter.next().unwrap();
                    output.push_str(grapheme);
                    if grapheme == "." {
                        output.push_str(segmenter.next().unwrap());
                    }
                    output.push('/');
                }
                output.push_str(&elements[elements.len() - 1]);
                return output;
            }
        }

        swd
    }

    pub fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            "SWD" => return Some(self.get_simplified_directory()),
            "MWD" => return Some(self.get_minimal_directory()),
            _ => (),
        }
        if let Some((name, variable)) = name.find("::").map(|pos| (&name[..pos], &name[pos + 2..]))
        {
            // If the parsed name contains the '::' pattern, then a namespace was
            // designated. Find it.
            match name {
                "c" | "color" => Colors::collect(variable).into_string(),
                "x" | "hex" => match u8::from_str_radix(variable, 16) {
                    Ok(c) => Some((c as char).to_string()),
                    Err(why) => {
                        eprintln!("ion: hex parse error: {}: {}", variable, why);
                        None
                    }
                },
                "env" => env::var(variable).map(Into::into).ok(),
                _ => {
                    if is_root() {
                        eprintln!("ion: root is not allowed to execute plugins");
                        return None;
                    }

                    if !self.has_plugin_support() {
                        eprintln!(
                            "ion: plugins are disabled. Considering enabling them with `let \
                             NS_PLUGINS = 1`"
                        );
                        return None;
                    }

                    // Attempt to obtain the given namespace from our lazily-generated map of
                    // namespaces.
                    if let Some(namespace) = STRING_NAMESPACES.get(name.into()) {
                        // Attempt to execute the given function from that namespace, and map it's
                        // results.
                        match namespace.execute(variable.into()) {
                            Ok(value) => value.map(Into::into),
                            Err(why) => {
                                eprintln!("ion: string namespace error: {}: {}", name, why);
                                None
                            }
                        }
                    } else {
                        eprintln!("ion: unsupported namespace: '{}'", name);
                        None
                    }
                }
            }
        } else {
            // Otherwise, it's just a simple variable name.
            self.variables
                .get(name)
                .cloned()
                .or_else(|| env::var(name).map(Into::into).ok())
        }
    }

    pub fn get_var_or_empty(&self, name: &str) -> Value { self.get_var(name).unwrap_or_default() }

    pub fn unset_var(&mut self, name: &str) -> Option<Value> { self.variables.remove(name) }

    pub fn get_vars<'a>(&'a self) -> impl Iterator<Item = Identifier> + 'a {
        self.variables
            .keys()
            .cloned()
            .chain(env::vars().map(|(k, _)| k.into()))
    }

    pub(crate) fn is_valid_variable_character(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '?'
    }

    pub(crate) fn is_valid_variable_name(name: &str) -> bool {
        name.chars().all(Variables::is_valid_variable_character)
    }

    pub(crate) fn tilde_expansion(&self, word: &str, dir_stack: &DirectoryStack) -> Option<String> {
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
            "" => if let Some(home) = env::home_dir() {
                return Some(home.to_string_lossy().to_string() + remainder);
            },
            "+" => if let Some(pwd) = self.get_var("PWD") {
                return Some(pwd.to_string() + remainder);
            } else if let Ok(pwd) = env::current_dir() {
                return Some(pwd.to_string_lossy().to_string() + remainder);
            },
            "-" => if let Some(oldpwd) = self.get_var("OLDPWD") {
                return Some(oldpwd.to_string() + remainder);
            },
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
                    Err(_) => if let Some(home) = self_sys::get_user_home(tilde_prefix) {
                        return Some(home + remainder);
                    },
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    pub(crate) fn is_hashmap_reference(key: &str) -> Option<(Identifier, Key)> {
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
    use parser::{expand_string, Expander};

    struct VariableExpander(pub Variables);

    impl Expander for VariableExpander {
        fn variable(&self, var: &str, _: bool) -> Option<Value> { self.0.get_var(var) }
    }

    #[test]
    fn undefined_variable_expands_to_empty_string() {
        let variables = Variables::default();
        let expanded = expand_string("$FOO", &VariableExpander(variables), false).join("");
        assert_eq!("", &expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let expanded = expand_string("$FOO", &VariableExpander(variables), false).join("");
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

    #[test]
    fn minimal_directory_var_should_compact_path() {
        let mut variables = Variables::default();
        variables.set_var("PWD", "/var/log/nix");
        assert_eq!(
            "v/l/nix",
            variables.get_var("MWD").expect("no value returned")
        );
    }

    #[test]
    fn minimal_directory_var_shouldnt_compact_path() {
        let mut variables = Variables::default();
        variables.set_var("PWD", "/var/log");
        assert_eq!(
            "/var/log",
            variables.get_var("MWD").expect("no value returned")
        );
    }
}
