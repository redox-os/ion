use super::{
    colors::Colors,
    directory_stack::DirectoryStack,
    flow_control::Function,
    plugins::namespaces::{self, StringNamespace},
    status::{FAILURE, SUCCESS},
};
use fnv::FnvHashMap;
use liner::Context;
use smallstring::SmallString;
use std::{
    env,
    io::{self, BufRead},
    mem
};
use sys::{self, geteuid, getpid, getuid, is_root, variables as self_sys};
use types::{
    Array, HashMap, Identifier, Key, Value,
};
use unicode_segmentation::UnicodeSegmentation;
use xdg::BaseDirectories;

lazy_static! {
    static ref STRING_NAMESPACES: FnvHashMap<Identifier, StringNamespace> = namespaces::collect();
}

#[derive(Clone, Debug)]
pub enum VariableType {
    Alias(Value),
    Array(Array),
    Function(Function),
    HashMap(HashMap),
    Variable(Value)
}

#[derive(Clone, Debug)]
pub struct Variables {
    flags:   u8,
    scopes:  Vec<FnvHashMap<Identifier, VariableType>>,
    current: usize,
}

impl Default for Variables {
    fn default() -> Self {
        let mut map: FnvHashMap<Identifier, VariableType> = FnvHashMap::with_capacity_and_hasher(64, Default::default());
        map.insert("DIRECTORY_STACK_SIZE".into(), VariableType::Variable("1000".into()));
        map.insert("HISTORY_SIZE".into(), VariableType::Variable("1000".into()));
        map.insert("HISTFILE_SIZE".into(), VariableType::Variable("100000".into()));
        map.insert(
            "PROMPT".into(),
            VariableType::Variable(
                "${x::1B}]0;${USER}: \
                 ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# \
                 ${c::reset}"
                    .into()
            ),
        );

        // Set the PID, UID, and EUID variables.
        map.insert(
            "PID".into(),
            VariableType::Variable(getpid().ok().map_or("?".into(), |id| id.to_string())),
        );
        map.insert(
            "UID".into(),
            VariableType::Variable(getuid().ok().map_or("?".into(), |id| id.to_string())),
        );
        map.insert(
            "EUID".into(),
            VariableType::Variable(geteuid().ok().map_or("?".into(), |id| id.to_string())),
        );

        // Initialize the HISTFILE variable
        if let Ok(base_dirs) = BaseDirectories::with_prefix("ion") {
            if let Ok(path) = base_dirs.place_data_file("history") {
                map.insert("HISTFILE".into(), VariableType::Variable(path.to_str().unwrap_or("?").into()));
                map.insert("HISTFILE_ENABLED".into(), VariableType::Variable("1".into()));
            }
        }

        map.insert("HISTORY_IGNORE".into(), VariableType::Array(array!["no_such_command", "whitespace", "duplicates"]));

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

        // Initialize the HOST variable
        env::set_var("HOST", &self_sys::get_host_name().unwrap_or("?".to_owned()));

        Variables {
            flags: 0,
            scopes: vec![map],
            current: 0
        }
    }
}

const PLUGIN: u8 = 1;

impl Variables {
    pub fn new_scope(&mut self) {
        self.current += 1;
        if self.current >= self.scopes.len() {
            self.scopes.push(FnvHashMap::with_capacity_and_hasher(64, Default::default()));
        }
    }
    pub fn pop_scope(&mut self) {
        self.scopes[self.current].clear();
        self.current -= 1;
    }
    pub fn pop_scopes<'a>(&'a mut self, index: usize) -> impl Iterator<Item = FnvHashMap<Identifier, VariableType>> + 'a {
        self.current = index;
        self.scopes.drain(index+1..)
    }
    pub fn append_scopes(&mut self, scopes: Vec<FnvHashMap<Identifier, VariableType>>) {
        self.scopes.drain(self.current+1..);
        self.current += scopes.len();
        self.scopes.extend(scopes);
    }
    pub fn scopes(&self) -> impl Iterator<Item = &FnvHashMap<Identifier, VariableType>> {
        let amount = self.scopes.len() - self.current - 1;
        self.scopes.iter().rev().skip(amount)
    }
    pub fn scopes_mut(&mut self) -> impl Iterator<Item = &mut FnvHashMap<Identifier, VariableType>> {
        let amount = self.scopes.len() - self.current - 1;
        self.scopes.iter_mut().rev().skip(amount)
    }
    pub fn index_scope_for_var(&self, name: &str) -> Option<usize> {
        let amount = self.scopes.len() - self.current - 1;
        for (i, scope) in self.scopes.iter().enumerate().rev().skip(amount) {
            if scope.contains_key(name) {
                return Some(i);
            }
        }
        None
    }
    pub fn shadow(&mut self, name: SmallString, value: VariableType) -> Option<VariableType> {
        self.scopes[self.current].insert(name, value)
    }
    pub fn lookup_any(&self, name: &str) -> Option<&VariableType> {
        for scope in self.scopes() {
            if let val @ Some(_) = scope.get(name) {
                return val;
            }
        }
        None
    }
    pub fn lookup_any_mut(&mut self, name: &str) -> Option<&mut VariableType> {
        for scope in self.scopes_mut() {
            if let val @ Some(_) = scope.get_mut(name) {
                return val;
            }
        }
        None
    }
    pub fn remove_any(&mut self, name: &str) -> Option<VariableType> {
        for scope in self.scopes_mut() {
            if let val @ Some(_) = scope.remove(name) {
                return val;
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

    pub(crate) fn is_valid_variable_name(name: &str) -> bool {
        name.chars().all(Variables::is_valid_variable_character)
    }

    pub(crate) fn is_valid_variable_character(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '?' || c == '.'
    }

    pub fn variables(&self) -> impl Iterator<Item = (&SmallString, &Value)> {
        self.scopes()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, val)| if let VariableType::Variable(val) = val {
                        Some((key, val))
                    } else {
                        None
                    })
            })
            .flatten()
    }
    pub fn unset_var(&mut self, name: &str) -> Option<Value> {
        match self.remove_any(name) {
            Some(VariableType::Variable(val)) => Some(val),
            _ => None
        }
    }
    pub fn get_var_or_empty(&self, name: &str) -> Value { self.get_var(name).unwrap_or_default() }
    pub fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            "MWD" => return Some(self.get_minimal_directory()),
            "SWD" => return Some(self.get_simplified_directory()),
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
                    if let Some(namespace) = STRING_NAMESPACES.get(name) {
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
            match self.lookup_any(name) {
                Some(VariableType::Variable(val)) => Some(val.clone()),
                _ => env::var(name).ok()
            }
        }
    }
    pub fn set_var(&mut self, name: &str, value: &str) {
        if !name.is_empty() {
            if value.is_empty() {
                self.unset_var(name);
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
                match self.lookup_any_mut(name) {
                    Some(VariableType::Variable(val)) => *val = value.into(),
                    _ => { self.shadow(name.into(), VariableType::Variable(value.into())); }
                }
            }
        }
    }

    pub fn insert_alias(&mut self, name: SmallString, value: Value) -> Option<Value> {
        match self.lookup_any_mut(&name) {
            Some(VariableType::Alias(val)) => Some(mem::replace(val, value)),
            _ => { self.shadow(name, VariableType::Alias(value)); None }
        }
    }
    pub fn get_alias(&self, name: &str) -> Option<Value> {
        match self.lookup_any(name) {
            Some(VariableType::Alias(val)) => Some(val.clone()),
            _ => None
        }
    }
    pub fn remove_alias(&mut self, name: &str) -> Option<Value> {
        match self.remove_any(name) {
            Some(VariableType::Alias(val)) => Some(val),
            _ => None
        }
    }
    pub fn aliases(&self) -> impl Iterator<Item = (&SmallString, &Value)> {
        self.scopes.iter().rev()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, val)| if let VariableType::Alias(val) = val {
                        Some((key, val))
                    } else {
                        None
                    })
            })
            .flatten()
    }

    pub fn insert_function(&mut self, name: SmallString, value: Function) -> Option<Function> {
        match self.lookup_any_mut(&name) {
            Some(VariableType::Function(val)) => Some(mem::replace(val, value)),
            _ => { self.shadow(name, VariableType::Function(value)); None }
        }
    }
    pub fn get_function(&self, name: &str) -> Option<Function> {
        match self.lookup_any(name) {
            Some(VariableType::Function(val)) => Some(val.clone()),
            _ => None
        }
    }
    pub fn remove_function(&mut self, name: &str) -> Option<Function> {
        match self.remove_any(name) {
            Some(VariableType::Function(val)) => Some(val),
            _ => None
        }
    }
    pub fn functions(&self) -> impl Iterator<Item = (&SmallString, &Function)> {
        self.scopes.iter().rev()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, val)| if let VariableType::Function(val) = val {
                        Some((key, val))
                    } else {
                        None
                    })
            })
            .flatten()
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
            let elements = swd
                .split('/')
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

    /// Obtains the value for the **SWD** variable.
    ///
    /// Useful for getting smaller prompts, this will produce a simplified variant of the
    /// working directory which the leading `HOME` prefix replaced with a tilde character.
    fn get_simplified_directory(&self) -> Value {
        self.get_var("PWD")
            .unwrap()
            .replace(&self.get_var("HOME").unwrap(), "~")
    }

    pub fn unset_array(&mut self, name: &str) -> Option<Array> {
        match self.remove_any(name) {
            Some(VariableType::Array(val)) => Some(val),
            _ => None
        }
    }

    pub fn get_array(&self, name: &str) -> Option<Array> {
        match self.lookup_any(name) {
            Some(VariableType::Array(val)) => Some(val.clone()),
            _ => None
        }
    }

    pub fn get_map(&self, name: &str) -> Option<HashMap> {
        match self.lookup_any(name) {
            Some(VariableType::HashMap(val)) => Some(val.clone()),
            _ => None
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_hashmap_value(&mut self, name: &str, key: &str, value: &str) {
        match self.lookup_any_mut(name) {
            Some(VariableType::HashMap(map)) => {
                map.insert(key.into(), value.into());
            },
            _ => {
                let mut map = HashMap::with_capacity_and_hasher(4, Default::default());
                map.insert(key.into(), value.into());
                self.shadow(name.into(), VariableType::HashMap(map));
            }
        }
    }

    pub fn set_array(&mut self, name: &str, value: Array) {
        if !name.is_empty() {
            if value.is_empty() {
                self.remove_any(name);
            } else {
                match self.lookup_any_mut(name) {
                    Some(VariableType::Array(val)) => *val = value,
                    _ => { self.shadow(name.into(), VariableType::Array(value)); }
                }
            }
        }
    }
    pub fn arrays(&self) -> impl Iterator<Item = (&SmallString, &Array)> {
        self.scopes.iter().rev()
            .map(|map| {
                map.iter()
                    .filter_map(|(key, val)| if let VariableType::Array(val) = val {
                        Some((key, val))
                    } else {
                        None
                    })
            })
            .flatten()
    }

    pub(crate) fn read<I: IntoIterator>(&mut self, args: I) -> i32
    where
        I::Item: AsRef<str>,
    {
        if sys::isatty(sys::STDIN_FILENO) {
            let mut con = Context::new();
            for arg in args.into_iter().skip(1) {
                match con.read_line(format!("{}=", arg.as_ref().trim()), None, &mut |_| {}) {
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

    pub(crate) fn disable_plugins(&mut self) { self.flags &= !PLUGIN; }

    pub(crate) fn enable_plugins(&mut self) { self.flags |= PLUGIN; }

    pub(crate) fn has_plugin_support(&self) -> bool { self.flags & PLUGIN == PLUGIN }
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
