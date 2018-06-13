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
    cell::{Cell, RefCell},
    io::{self, BufRead},
};
use sys::{self, geteuid, getpid, getuid, is_root, variables as self_sys};
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
pub struct Variables<'a> {
    pub parent:    Option<&'a Variables<'a>>,
    pub hashmaps:  RefCell<HashMapVariableContext>,
    pub arrays:    RefCell<ArrayVariableContext>,
    pub variables: RefCell<VariableContext>,
    pub aliases:   RefCell<VariableContext>,
    pub functions: RefCell<FnvHashMap<Identifier, Function>>,
    flags:         Cell<u8>,
}

impl<'a> Default for Variables<'a> {
    fn default() -> Self {
        let mut map_vars = FnvHashMap::with_capacity_and_hasher(64, Default::default());
        map_vars.insert("DIRECTORY_STACK_SIZE".into(), "1000".into());
        map_vars.insert("HISTORY_SIZE".into(), "1000".into());
        map_vars.insert("HISTFILE_SIZE".into(), "100000".into());
        map_vars.insert(
            "PROMPT".into(),
            "${x::1B}]0;${USER}: \
             ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# \
             ${c::reset}"
                .into(),
        );

        // Set the PID, UID, and EUID variables.
        map_vars.insert(
            "PID".into(),
            getpid().ok().map_or("?".into(), |id| id.to_string()),
        );
        map_vars.insert(
            "UID".into(),
            getuid().ok().map_or("?".into(), |id| id.to_string()),
        );
        map_vars.insert(
            "EUID".into(),
            geteuid().ok().map_or("?".into(), |id| id.to_string()),
        );

        // Initialize the HISTFILE variable
        if let Ok(base_dirs) = BaseDirectories::with_prefix("ion") {
            if let Ok(path) = base_dirs.place_data_file("history") {
                map_vars.insert("HISTFILE".into(), path.to_str().unwrap_or("?").into());
                map_vars.insert("HISTFILE_ENABLED".into(), "1".into());
            }
        }

        let mut map_arrays = FnvHashMap::with_capacity_and_hasher(64, Default::default());
        map_arrays.insert("HISTORY_IGNORE".into(), array!["no_such_command", "whitespace", "duplicates"]);

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
            parent:    None,
            hashmaps:  RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            arrays:    RefCell::new(map_arrays),
            variables: RefCell::new(map_vars),
            aliases:   RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            functions: RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            flags:     Cell::new(0),
        }
    }
}

const PLUGIN: u8 = 1;

macro_rules! descend_scopes {
    (clone $var:ident) => {
        Some($var.clone())
    };
    (no_clone $var:ident) => {
        Some($var)
    };
    (ref $name:expr) => {
        &$name
    };
    (val $name:expr) => {
        $name
    };
    (lookup, $self:ident.$map:ident.$borrow:ident().$lookup:ident($name:expr) else $fallback:block, $clone:tt) => {
        {
            let mut me = $self;
            loop {
                if let Some(var) = me.$map.$borrow().$lookup($name) {
                    break descend_scopes!($clone var);
                }
                match me.parent {
                    Some(parent) => me = parent,
                    None => break $fallback
                }
            }
        }
    };
    (insert, $self:ident.$map:ident, $borrow:tt $name:expr, $value:expr) => {
        {
            let mut me = $self;
            loop {
                let mut map = me.$map.borrow_mut();
                if map.contains_key(descend_scopes!($borrow $name)) {
                    break map.insert($name.into(), $value.into());
                }
                match me.parent {
                    Some(parent) => me = parent,
                    None => {
                        // It wasn't found, insert new at current scope
                        drop(map);
                        break $self.$map.borrow_mut().insert($name.into(), $value.into());
                    }
                }
            }
        }
    }
}

impl<'a> Variables<'a> {
    pub(crate) fn new_scope<'b>(&'b self) -> Variables<'b> {
        Variables {
            flags:     self.flags.clone(),
            parent:    Some(self),
            hashmaps:  RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            arrays:    RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            variables: RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            aliases:   RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
            functions: RefCell::new(FnvHashMap::with_capacity_and_hasher(64, Default::default())),
        }
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

    pub fn strings(&self) -> Vec<SmallString> {
        let vars = self.variables.borrow_mut();
        vars.keys().cloned().chain(env::vars().map(|(k, _)| k.into())).collect()
    }

    pub fn unset_var(&self, name: &str) -> Option<Value> {
        self.variables.borrow_mut().remove(name)
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
            // Travel down the scopes and look for it.
            descend_scopes!(lookup, self.variables.borrow().get(name) else { env::var(name).ok() }, clone)
        }
    }
    pub fn insert_alias(&self, name: SmallString, value: Value) -> Option<Value> {
        descend_scopes!(insert, self.aliases, ref name, value)
    }
    pub fn get_alias(&self, name: &str) -> Option<Value> {
        descend_scopes!(lookup, self.aliases.borrow().get(name) else { None }, clone)
    }
    pub fn remove_alias(&self, name: &str) -> Option<Value> {
        descend_scopes!(lookup, self.aliases.borrow_mut().remove(name) else { None }, no_clone)
    }
    pub fn insert_function(&self, name: SmallString, value: Function) -> Option<Function> {
        descend_scopes!(insert, self.functions, ref name, value)
    }
    pub fn get_function(&self, name: &str) -> Option<Function> {
        descend_scopes!(lookup, self.functions.borrow().get(name) else { None }, clone)
    }
    pub fn remove_function(&self, name: &str) -> Option<Function> {
        descend_scopes!(lookup, self.functions.borrow_mut().remove(name) else { None }, no_clone)
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
                .split("/")
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

    pub fn unset_array(&self, name: &str) -> Option<Array> {
        let mut me = self;
        loop {
            if let Some(var) = me.arrays.borrow_mut().remove(name) {
                return Some(var);
            }
            match me.parent {
                Some(parent) => me = parent,
                None => break
            }
        }
        None
    }

    pub fn get_array(&self, name: &str) -> Option<Array> {
        descend_scopes!(lookup, self.arrays.borrow().get(name) else { None }, clone)
    }

    pub fn get_map(&self, name: &str) -> Option<HashMap> {
        descend_scopes!(lookup, self.hashmaps.borrow().get(name) else { None }, clone)
    }

    #[allow(dead_code)]
    pub(crate) fn set_hashmap_value(&self, name: &str, key: &str, value: &str) {
        let mut me = self;
        loop {
            if let Some(map) = me.hashmaps.borrow_mut().get_mut(name) {
                map.insert(key.into(), value.into());
                break;
            }
            match me.parent {
                Some(parent) => me = parent,
                None => {
                    let mut map = HashMap::with_capacity_and_hasher(4, Default::default());
                    map.insert(key.into(), value.into());
                    self.hashmaps.borrow_mut().insert(name.into(), map);
                    break;
                }
            }
        }
    }

    pub fn set_array(&self, name: &str, value: Array) {
        if !name.is_empty() {
            if value.is_empty() {
                descend_scopes!(lookup, self.arrays.borrow_mut().remove(name) else { None }, no_clone);
            } else {
                descend_scopes!(insert, self.arrays, val name, value);
            }
        }
    }

    pub fn set_var(&self, name: &str, value: &str) {
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
                descend_scopes!(insert, self.variables, val name, value);
            }
        }
    }

    pub(crate) fn read<I: IntoIterator>(&self, args: I) -> i32
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

    pub(crate) fn disable_plugins(&self) { self.flags.set(self.flags.get() & 255 ^ PLUGIN); }

    pub(crate) fn enable_plugins(&self) { self.flags.set(self.flags.get() | PLUGIN) }

    pub(crate) fn has_plugin_support(&self) -> bool { self.flags.get() & PLUGIN != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::{expand_string, Expander};

    struct VariableExpander(pub Variables<'static>);

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
        let variables = Variables::default();
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
        let variables = Variables::default();
        variables.set_var("PWD", "/var/log/nix");
        assert_eq!(
            "v/l/nix",
            variables.get_var("MWD").expect("no value returned")
        );
    }

    #[test]
    fn minimal_directory_var_shouldnt_compact_path() {
        let variables = Variables::default();
        variables.set_var("PWD", "/var/log");
        assert_eq!(
            "/var/log",
            variables.get_var("MWD").expect("no value returned")
        );
    }
}
