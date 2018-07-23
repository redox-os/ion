use super::{
    colors::Colors,
    directory_stack::DirectoryStack,
    flow_control::Function,
    status::{FAILURE, SUCCESS},
};
use fnv::FnvHashMap;
use liner::Context;
use std::{
    any::TypeId,
    env, fmt,
    io::{self, BufRead},
    mem,
    ops::{Deref, DerefMut},
};
use sys::{self, env as sys_env, geteuid, getpid, getuid, variables as self_sys};
use types::{self, Array};
use unicode_segmentation::UnicodeSegmentation;
use xdg::BaseDirectories;

#[derive(Clone, Debug, PartialEq)]
pub enum VariableType {
    Str(types::Str),
    Alias(types::Alias),
    Array(types::Array),
    HashMap(types::HashMap),
    BTreeMap(types::BTreeMap),
    Function(Function),
    None,
}

impl From<VariableType> for types::Str {
    fn from(var: VariableType) -> Self {
        match var {
            VariableType::Str(string) => string,
            _ => types::Str::with_capacity(0),
        }
    }
}

impl From<VariableType> for types::Alias {
    fn from(var: VariableType) -> Self {
        match var {
            VariableType::Alias(alias) => alias,
            _ => types::Alias::empty(),
        }
    }
}

impl From<VariableType> for types::Array {
    fn from(var: VariableType) -> Self {
        match var {
            VariableType::Array(array) => array,
            _ => types::Array::with_capacity(0),
        }
    }
}

impl From<VariableType> for types::HashMap {
    fn from(var: VariableType) -> Self {
        match var {
            VariableType::HashMap(hash_map) => hash_map,
            _ => types::HashMap::with_capacity_and_hasher(0, Default::default()),
        }
    }
}

impl From<VariableType> for types::BTreeMap {
    fn from(var: VariableType) -> Self {
        match var {
            VariableType::BTreeMap(btree_map) => btree_map,
            _ => types::BTreeMap::new(),
        }
    }
}

impl From<VariableType> for Function {
    fn from(var: VariableType) -> Self {
        match var {
            VariableType::Function(function) => function,
            _ => Function::new(
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
            ),
        }
    }
}

impl<'a> From<&'a str> for VariableType {
    fn from(string: &'a str) -> Self { VariableType::Str(string.into()) }
}

impl From<types::Str> for VariableType {
    fn from(string: types::Str) -> Self { VariableType::Str(string) }
}

impl From<String> for VariableType {
    fn from(string: String) -> Self { VariableType::Str(string.into()) }
}

impl From<types::Alias> for VariableType {
    fn from(alias: types::Alias) -> Self { VariableType::Alias(alias) }
}

impl From<types::Array> for VariableType {
    fn from(array: types::Array) -> Self { VariableType::Array(array) }
}

impl From<types::HashMap> for VariableType {
    fn from(hmap: types::HashMap) -> Self { VariableType::HashMap(hmap) }
}

impl From<types::BTreeMap> for VariableType {
    fn from(bmap: types::BTreeMap) -> Self { VariableType::BTreeMap(bmap) }
}

impl From<Function> for VariableType {
    fn from(function: Function) -> Self { VariableType::Function(function) }
}

impl fmt::Display for VariableType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            VariableType::Str(ref str_) => write!(f, "{}", str_),
            VariableType::Alias(ref alias) => write!(f, "{}", **alias),
            VariableType::Array(ref array) => write!(f, "{}", array.join(" ")),
            VariableType::HashMap(ref map) => {
                let mut format =
                    map.into_iter()
                        .fold(String::new(), |mut format, (_, var_type)| {
                            format.push_str(&format!("{}", var_type));
                            format.push(' ');
                            format
                        });
                format.pop();
                write!(f, "{}", format)
            }
            VariableType::BTreeMap(ref map) => {
                let mut format =
                    map.into_iter()
                        .fold(String::new(), |mut format, (_, var_type)| {
                            format.push_str(&format!("{}", var_type));
                            format.push(' ');
                            format
                        });
                format.pop();
                write!(f, "{}", format)
            }
            _ => write!(f, ""),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scope {
    vars: FnvHashMap<types::Str, VariableType>,
    /// This scope is on a namespace boundary.
    /// Any previous scopes need to be accessed through `super::`.
    namespace: bool,
}

impl Deref for Scope {
    type Target = FnvHashMap<types::Str, VariableType>;

    fn deref(&self) -> &Self::Target { &self.vars }
}

impl DerefMut for Scope {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.vars }
}

#[derive(Clone, Debug)]
pub struct Variables {
    flags:   u8,
    scopes:  Vec<Scope>,
    current: usize,
}

impl Default for Variables {
    fn default() -> Self {
        let mut map: FnvHashMap<types::Str, VariableType> =
            FnvHashMap::with_capacity_and_hasher(64, Default::default());
        map.insert(
            "DIRECTORY_STACK_SIZE".into(),
            VariableType::Str("1000".into()),
        );
        map.insert("HISTORY_SIZE".into(), VariableType::Str("1000".into()));
        map.insert("HISTFILE_SIZE".into(), VariableType::Str("100000".into()));
        map.insert(
            "PROMPT".into(),
            VariableType::Str(
                "${x::1B}]0;${USER}: \
                 ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# \
                 ${c::reset}"
                    .into(),
            ),
        );

        // Set the PID, UID, and EUID variables.
        map.insert(
            "PID".into(),
            VariableType::Str(getpid().ok().map_or("?".into(), |id| id.to_string().into())),
        );
        map.insert(
            "UID".into(),
            VariableType::Str(getuid().ok().map_or("?".into(), |id| id.to_string().into())),
        );
        map.insert(
            "EUID".into(),
            VariableType::Str(
                geteuid()
                    .ok()
                    .map_or("?".into(), |id| id.to_string().into()),
            ),
        );

        // Initialize the HISTFILE variable
        if let Ok(base_dirs) = BaseDirectories::with_prefix("ion") {
            if let Ok(path) = base_dirs.place_data_file("history") {
                map.insert(
                    "HISTFILE".into(),
                    VariableType::Str(path.to_str().unwrap_or("?").into()),
                );
                map.insert("HISTFILE_ENABLED".into(), VariableType::Str("1".into()));
            }
        }

        map.insert(
            "HISTORY_IGNORE".into(),
            VariableType::Array(array!["no_such_command", "whitespace", "duplicates"]),
        );

        // Initialize the HOME variable
        sys_env::home_dir().map_or_else(
            || env::set_var("HOME", "?"),
            |path| env::set_var("HOME", path.to_str().unwrap_or("?")),
        );

        // Initialize the HOST variable
        env::set_var(
            "HOST",
            &self_sys::get_host_name().unwrap_or_else(|| "?".to_owned()),
        );

        Variables {
            flags:   0,
            scopes:  vec![Scope {
                vars:      map,
                namespace: false,
            }],
            current: 0,
        }
    }
}

impl Variables {
    pub fn new_scope(&mut self, namespace: bool) {
        self.current += 1;
        if self.current >= self.scopes.len() {
            self.scopes.push(Scope {
                vars: FnvHashMap::with_capacity_and_hasher(64, Default::default()),
                namespace,
            });
        } else {
            self.scopes[self.current].namespace = namespace;
        }
    }

    pub fn pop_scope(&mut self) {
        self.scopes[self.current].clear();
        self.current -= 1;
    }

    pub fn pop_scopes<'a>(&'a mut self, index: usize) -> impl Iterator<Item = Scope> + 'a {
        self.current = index;
        self.scopes.drain(index + 1..)
    }

    pub fn append_scopes(&mut self, scopes: Vec<Scope>) {
        self.scopes.drain(self.current + 1..);
        self.current += scopes.len();
        self.scopes.extend(scopes);
    }

    pub fn scopes(&self) -> impl Iterator<Item = &Scope> {
        let amount = self.scopes.len() - self.current - 1;
        self.scopes.iter().rev().skip(amount)
    }

    pub fn scopes_mut(&mut self) -> impl Iterator<Item = &mut Scope> {
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

    pub fn shadow(&mut self, name: &str, value: VariableType) -> Option<VariableType> {
        self.scopes[self.current].insert(name.into(), value)
    }

    pub fn get_ref(&self, mut name: &str) -> Option<&VariableType> {
        let mut up_namespace: isize = 0;
        if name.starts_with("global::") {
            name = &name["global::".len()..];
            // Go up as many namespaces as possible
            up_namespace = self.scopes().filter(|scope| scope.namespace).count() as isize;
        } else {
            while name.starts_with("super::") {
                name = &name["super::".len()..];
                up_namespace += 1;
            }
        }
        for scope in self.scopes() {
            match scope.get(name) {
                val @ Some(VariableType::Function(_)) => return val,
                val @ Some(_) if up_namespace == 0 => return val,
                _ => (),
            }
            if scope.namespace {
                up_namespace -= 1;
            }
        }
        None
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut VariableType> {
        if name.starts_with("super::") || name.starts_with("global::") {
            // Cannot mutate outer namespace
            return None;
        }
        for scope in self.scopes_mut() {
            let exit = scope.namespace;
            if let val @ Some(_) = scope.get_mut(name) {
                return val;
            }
            if exit {
                break;
            }
        }
        None
    }

    pub fn remove_variable(&mut self, name: &str) -> Option<VariableType> {
        for scope in self.scopes_mut() {
            if let val @ Some(_) = scope.remove(name) {
                return val;
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
            "" => if let Some(home) = sys_env::home_dir() {
                return Some(home.to_string_lossy().to_string() + remainder);
            },
            "+" => {
                return Some(match env::var("PWD") {
                    Ok(var) => var + remainder,
                    _ => ["?", remainder].concat(),
                })
            }
            "-" => if let Some(oldpwd) = self.get::<types::Str>("OLDPWD") {
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
        c.is_alphanumeric() || c == '_' || c == '?' || c == '.' || c == '-' || c == '+'
    }

    pub fn string_vars(&self) -> impl Iterator<Item = (&types::Str, &types::Str)> {
        self.scopes()
            .map(|map| {
                map.iter().filter_map(|(key, val)| {
                    if let VariableType::Str(val) = val {
                        Some((key, val))
                    } else {
                        None
                    }
                })
            })
            .flat_map(|f| f)
    }

    pub fn get_str_or_empty(&self, name: &str) -> types::Str {
        self.get::<types::Str>(name).unwrap_or_default()
    }

    pub fn get<T: Clone + From<VariableType> + 'static>(&self, name: &str) -> Option<T> {
        let specified_type = TypeId::of::<T>();

        if specified_type == TypeId::of::<types::Str>() {
            match name {
                "MWD" => return Some(T::from(VariableType::Str(self.get_minimal_directory()))),
                "SWD" => return Some(T::from(VariableType::Str(self.get_simplified_directory()))),
                _ => (),
            }
            // If the parsed name contains the '::' pattern, then a namespace was
            // designated. Find it.
            match name.find("::").map(|pos| (&name[..pos], &name[pos + 2..])) {
                Some(("c", variable)) | Some(("color", variable)) => Colors::collect(variable)
                    .into_string()
                    .map(|s| T::from(VariableType::Str(s.into()))),
                Some(("x", variable)) | Some(("hex", variable)) => {
                    match u8::from_str_radix(variable, 16) {
                        Ok(c) => Some(T::from(VariableType::Str((c as char).to_string().into()))),
                        Err(why) => {
                            eprintln!("ion: hex parse error: {}: {}", variable, why);
                            None
                        }
                    }
                }
                Some(("env", variable)) => env::var(variable)
                    .map(Into::into)
                    .ok()
                    .map(|s| T::from(VariableType::Str(s))),
                Some(("super", _)) | Some(("global", _)) | None => {
                    // Otherwise, it's just a simple variable name.
                    match self.get_ref(name) {
                        Some(VariableType::Str(val)) => {
                            Some(T::from(VariableType::Str(val.clone())))
                        }
                        _ => env::var(name)
                            .ok()
                            .map(|s| T::from(VariableType::Str(s.into()))),
                    }
                }
                Some((..)) => {
                    eprintln!("ion: unsupported namespace: '{}'", name);
                    None
                }
            }
        } else if specified_type == TypeId::of::<types::Alias>() {
            match self.get_ref(name) {
                Some(VariableType::Alias(alias)) => {
                    Some(T::from(VariableType::Alias((*alias).clone())))
                }
                _ => None,
            }
        } else if specified_type == TypeId::of::<types::Array>() {
            match self.get_ref(name) {
                Some(VariableType::Array(array)) => {
                    Some(T::from(VariableType::Array(array.clone())))
                }
                _ => None,
            }
        } else if specified_type == TypeId::of::<types::HashMap>() {
            match self.get_ref(name) {
                Some(VariableType::HashMap(hmap)) => {
                    Some(T::from(VariableType::HashMap(hmap.clone())))
                }
                _ => None,
            }
        } else if specified_type == TypeId::of::<types::BTreeMap>() {
            match self.get_ref(name) {
                Some(VariableType::BTreeMap(bmap)) => {
                    Some(T::from(VariableType::BTreeMap(bmap.clone())))
                }
                _ => None,
            }
        } else if specified_type == TypeId::of::<Function>() {
            match self.get_ref(name) {
                Some(VariableType::Function(func)) => {
                    Some(T::from(VariableType::Function(func.clone())))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn set<T: Into<VariableType>>(&mut self, name: &str, var: T) {
        let var = var.into();

        enum UpperAction {
            Remove,
            Shadow,
        }

        enum Action<'a> {
            Upper(UpperAction),
            Alias(&'a mut types::Alias),
            Str(&'a mut types::Str),
            Array(&'a mut types::Array),
            Function(&'a mut Function),
            HashMap(&'a mut types::HashMap),
        }

        macro_rules! handle_type {
            ($name:tt, $input:ty, $preferred:tt) => {
                fn $name<'a>(
                    name: &str,
                    var: &VariableType,
                    input: &'a mut $input,
                ) -> Option<Action<'a>> {
                    if !name.is_empty() {
                        match var {
                            VariableType::$preferred(var_value) => if var_value.is_empty() {
                                Some(Action::Upper(UpperAction::Remove))
                            } else {
                                Some(Action::$preferred(input))
                            },
                            _ => Some(Action::Upper(UpperAction::Shadow)),
                        }
                    } else {
                        None
                    }
                }
            };
        }

        handle_type!(string_action, types::Str, Str);
        handle_type!(alias_action, types::Alias, Alias);
        handle_type!(array_action, types::Array, Array);
        handle_type!(hashmap_action, types::HashMap, HashMap);
        handle_type!(function_action, Function, Function);

        let upper_action = {
            let action = match self.get_mut(&name) {
                Some(VariableType::Str(ref mut str_)) => string_action(name, &var, str_),
                Some(VariableType::Alias(ref mut alias)) => alias_action(name, &var, alias),
                Some(VariableType::Array(ref mut array)) => array_action(name, &var, array),
                Some(VariableType::HashMap(ref mut map)) => hashmap_action(name, &var, map),
                Some(VariableType::Function(ref mut func)) => function_action(name, &var, func),
                _ => Some(Action::Upper(UpperAction::Shadow)),
            };

            macro_rules! handle_action {
                ($value:ident, $variant:tt) => {{
                    if let VariableType::$variant(mut with) = var {
                        mem::replace($value, with);
                        None
                    } else {
                        unreachable!();
                    }
                }};
            }

            match action {
                Some(Action::Upper(action)) => Some((action, var)),
                Some(Action::Alias(alias)) => handle_action!(alias, Alias),
                Some(Action::Array(array)) => handle_action!(array, Array),
                Some(Action::Str(str_)) => handle_action!(str_, Str),
                Some(Action::Function(func)) => handle_action!(func, Function),
                Some(Action::HashMap(hmap)) => handle_action!(hmap, HashMap),
                None => None,
            }
        };

        match upper_action {
            Some((UpperAction::Remove, _)) => {
                self.remove_variable(name);
            }
            Some((UpperAction::Shadow, var)) => {
                self.shadow(name, var);
            }
            None => (),
        }
    }

    pub fn aliases(&self) -> impl Iterator<Item = (&types::Str, &types::Str)> {
        self.scopes
            .iter()
            .rev()
            .map(|map| {
                map.iter().filter_map(|(key, possible_alias)| {
                    if let VariableType::Alias(alias) = possible_alias {
                        Some((key, &**alias))
                    } else {
                        None
                    }
                })
            })
            .flat_map(|f| f)
    }

    pub fn functions(&self) -> impl Iterator<Item = (&types::Str, &Function)> {
        self.scopes
            .iter()
            .rev()
            .map(|map| {
                map.iter().filter_map(|(key, val)| {
                    if let VariableType::Function(val) = val {
                        Some((key, val))
                    } else {
                        None
                    }
                })
            })
            .flat_map(|f| f)
    }

    /// Obtains the value for the **MWD** variable.
    ///
    /// Further minimizes the directory path in the same manner that Fish does by default.
    /// That is, if more than two parents are visible in the path, all parent directories
    /// of the current directory will be reduced to a single character.
    fn get_minimal_directory(&self) -> types::Str {
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
                let mut output = types::Str::new();
                for element in &elements[..elements.len() - 1] {
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
    fn get_simplified_directory(&self) -> types::Str {
        let home = match self.get::<types::Str>("HOME") {
            Some(string) => string,
            None => "?".into(),
        };

        env::var("PWD").unwrap().replace(&*home, "~").into()
    }

    pub fn arrays(&self) -> impl Iterator<Item = (&types::Str, &types::Array)> {
        self.scopes
            .iter()
            .rev()
            .map(|map| {
                map.iter().filter_map(|(key, val)| {
                    if let VariableType::Array(val) = val {
                        Some((key, val))
                    } else {
                        None
                    }
                })
            })
            .flat_map(|f| f)
    }

    pub(crate) fn read<I: IntoIterator>(&mut self, args: I) -> i32
    where
        I::Item: AsRef<str>,
    {
        if sys::isatty(sys::STDIN_FILENO) {
            let mut con = Context::new();
            for arg in args.into_iter().skip(1) {
                match con.read_line(format!("{}=", arg.as_ref().trim()), None, &mut |_| {}) {
                    Ok(buffer) => {
                        self.set(arg.as_ref(), buffer.trim());
                    }
                    Err(_) => return FAILURE,
                }
            }
        } else {
            let stdin = io::stdin();
            let handle = stdin.lock();
            let mut lines = handle.lines();
            for arg in args.into_iter().skip(1) {
                if let Some(Ok(line)) = lines.next() {
                    self.set(arg.as_ref(), line.trim());
                }
            }
        }
        SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parser::{expand_string, Expander};

    struct VariableExpander(pub Variables);

    impl Expander for VariableExpander {
        fn string(&self, var: &str, _: bool) -> Option<types::Str> { self.0.get::<types::Str>(var) }
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
        variables.set("FOO", "BAR");
        let expanded = expand_string("$FOO", &VariableExpander(variables), false).join("");
        assert_eq!("BAR", &expanded);
    }

    use std::sync::Mutex;
    lazy_static! {
        static ref ENVLOCK: Mutex<()> = Mutex::new(());
    }

    #[test]
    fn minimal_directory_var_should_compact_path() {
        // Make sure we dont read the other tests writes to env
        let _guard = ENVLOCK.lock().unwrap();
        let variables = Variables::default();
        env::set_var("PWD", "/var/log/nix");
        assert_eq!(
            types::Str::from("v/l/nix"),
            match variables.get::<types::Str>("MWD") {
                Some(string) => string,
                None => panic!("no value returned"),
            }
        );
    }

    #[test]
    fn minimal_directory_var_shouldnt_compact_path() {
        // Make sure we dont read the other tests writes to env
        let _guard = ENVLOCK.lock().unwrap();
        let variables = Variables::default();
        env::set_var("PWD", "/var/log");
        assert_eq!(
            types::Str::from("/var/log"),
            match variables.get::<types::Str>("MWD") {
                Some(string) => string,
                None => panic!("no value returned"),
            }
        );
    }
}
