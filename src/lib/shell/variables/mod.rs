mod math;
mod modification;

pub use self::{
    math::{EuclDiv, OpError, Pow},
    modification::Modifications,
};
use super::{
    colors::Colors,
    flow_control::Function,
    status::{FAILURE, SUCCESS},
};
use crate::{
    sys::{self, env as sys_env, geteuid, getpid, getuid, variables as self_sys},
    types::{self, Array},
};
use hashbrown::HashMap;
use liner::Context;
use std::{
    env, fmt,
    io::{self, BufRead},
    mem,
    ops::{Deref, DerefMut},
};
use unicode_segmentation::UnicodeSegmentation;
use xdg::BaseDirectories;

#[derive(Clone, Debug, PartialEq)]
pub enum Value<'a> {
    Str(types::Str),
    Alias(types::Alias),
    Array(types::Array),
    HashMap(types::HashMap<'a>),
    BTreeMap(types::BTreeMap<'a>),
    Function(Function<'a>),
    None,
}

macro_rules! type_from_value {
    ($to:ty : $variant:ident else $defaultmethod:ident($($args:expr),*)) => {
        impl<'a> From<Value<'a>> for $to {
            fn from(var: Value<'a>) -> Self {
                match var {
                    Value::$variant(inner) => inner,
                    _ => <$to>::$defaultmethod($($args),*),
                }
            }
        }

        impl<'a> From<Value<'a>> for Option<$to> {
            fn from(var: Value<'a>) -> Self {
                match var {
                    Value::$variant(inner) => Some(inner),
                    _ => None,
                }
            }
        }

        impl<'a, 'b> From<&'b Value<'a>> for Option<&'b $to> {
            fn from(var: &'b Value<'a>) -> Self {
                match *var {
                    Value::$variant(ref inner) => Some(inner),
                    _ => None,
                }
            }
        }

        impl<'a, 'b> From<&'b mut Value<'a>> for Option<&'b mut $to> {
            fn from(var: &'b mut Value<'a>) -> Self {
                match *var {
                    Value::$variant(ref mut inner) => Some(inner),
                    _ => None,
                }
            }
        }
    }
}

type_from_value!(types::Str : Str else with_capacity(0));
type_from_value!(types::Alias : Alias else empty());
type_from_value!(types::Array : Array else with_capacity(0));
type_from_value!(types::HashMap<'a> : HashMap else with_capacity_and_hasher(0, Default::default()));
type_from_value!(types::BTreeMap<'a> : BTreeMap else new());
type_from_value!(Function<'a> : Function else
    new(
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default()
    )
);

macro_rules! eq {
    ($lhs:ty : $variant:ident) => {
        impl<'a> PartialEq<Value<'a>> for $lhs {
            fn eq(&self, other: &Value<'a>) -> bool {
                match other {
                    Value::$variant(ref inner) => inner == self,
                    _ => false,
                }
            }
        }
    };
}

eq!(types::Str: Str);
eq!(types::Alias: Alias);
eq!(types::Array: Array);
eq!(types::HashMap<'a>: HashMap);
eq!(types::BTreeMap<'a>: BTreeMap);
eq!(Function<'a>: Function);

impl<'a> Eq for Value<'a> {}

// this one’s only special because of the lifetime parameter
impl<'a, 'b> From<&'b str> for Value<'a> {
    fn from(string: &'b str) -> Self { Value::Str(string.into()) }
}

macro_rules! value_from_type {
    ($arg:ident: $from:ty => $variant:ident($inner:expr)) => {
        impl<'a> From<$from> for Value<'a> {
            fn from($arg: $from) -> Self { Value::$variant($inner) }
        }
    };
}

value_from_type!(string: types::Str => Str(string));
value_from_type!(string: String => Str(string.into()));
value_from_type!(alias: types::Alias => Alias(alias));
value_from_type!(array: types::Array => Array(array));
value_from_type!(hmap: types::HashMap<'a> => HashMap(hmap));
value_from_type!(bmap: types::BTreeMap<'a> => BTreeMap(bmap));
value_from_type!(function: Function<'a> => Function(function));

impl<'a> fmt::Display for Value<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Value::Str(ref str_) => write!(f, "{}", str_),
            Value::Alias(ref alias) => write!(f, "{}", **alias),
            Value::Array(ref array) => write!(f, "{}", array.join(" ")),
            Value::HashMap(ref map) => {
                let format = map
                    .iter()
                    .map(|(_, var_type)| format!("{}", var_type))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "{}", format)
            }
            Value::BTreeMap(ref map) => {
                let format = map
                    .iter()
                    .map(|(_, var_type)| format!("{}", var_type))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "{}", format)
            }
            _ => write!(f, ""),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scope<'a> {
    vars: HashMap<types::Str, Value<'a>>,
    /// This scope is on a namespace boundary.
    /// Any previous scopes need to be accessed through `super::`.
    namespace: bool,
}

impl<'a> Deref for Scope<'a> {
    type Target = HashMap<types::Str, Value<'a>>;

    fn deref(&self) -> &Self::Target { &self.vars }
}

impl<'a> DerefMut for Scope<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.vars }
}

#[derive(Clone, Debug)]
pub struct Variables<'a> {
    flags:   u8,
    scopes:  Vec<Scope<'a>>,
    current: usize,
}

impl<'a> Default for Variables<'a> {
    fn default() -> Self {
        let mut map: HashMap<types::Str, Value> = HashMap::with_capacity(64);
        map.insert("DIRECTORY_STACK_SIZE".into(), Value::Str("1000".into()));
        map.insert("HISTORY_SIZE".into(), Value::Str("1000".into()));
        map.insert("HISTFILE_SIZE".into(), Value::Str("100000".into()));
        map.insert(
            "PROMPT".into(),
            Value::Str(
                "${x::1B}]0;${USER}: \
                 ${PWD}${x::07}${c::0x55,bold}${USER}${c::default}:${c::0x4B}${SWD}${c::default}# \
                 ${c::reset}"
                    .into(),
            ),
        );

        // Set the PID, UID, and EUID variables.
        map.insert(
            "PID".into(),
            Value::Str(getpid().ok().map_or("?".into(), |id| id.to_string().into())),
        );
        map.insert(
            "UID".into(),
            Value::Str(getuid().ok().map_or("?".into(), |id| id.to_string().into())),
        );
        map.insert(
            "EUID".into(),
            Value::Str(geteuid().ok().map_or("?".into(), |id| id.to_string().into())),
        );

        // Initialize the HISTFILE variable
        if let Ok(base_dirs) = BaseDirectories::with_prefix("ion") {
            if let Ok(path) = base_dirs.place_data_file("history") {
                map.insert("HISTFILE".into(), Value::Str(path.to_str().unwrap_or("?").into()));
                map.insert("HISTFILE_ENABLED".into(), Value::Str("1".into()));
            }
        }

        // History Timestamps enabled variable, disabled by default
        map.insert("HISTORY_TIMESTAMP".into(), Value::Str("0".into()));

        map.insert(
            "HISTORY_IGNORE".into(),
            Value::Array(array!["no_such_command", "whitespace", "duplicates"]),
        );

        map.insert("CDPATH".into(), Value::Array(Array::new()));

        // Initialize the HOME variable
        sys_env::home_dir().map_or_else(
            || env::set_var("HOME", "?"),
            |path| env::set_var("HOME", path.to_str().unwrap_or("?")),
        );

        // Initialize the HOST variable
        env::set_var("HOST", &self_sys::get_host_name().unwrap_or_else(|| "?".to_owned()));

        Variables { flags: 0, scopes: vec![Scope { vars: map, namespace: false }], current: 0 }
    }
}

impl<'a> Variables<'a> {
    pub fn new_scope(&mut self, namespace: bool) {
        self.current += 1;
        if self.current >= self.scopes.len() {
            self.scopes.push(Scope { vars: HashMap::with_capacity(64), namespace });
        } else {
            self.scopes[self.current].namespace = namespace;
        }
    }

    pub fn pop_scope(&mut self) {
        self.scopes[self.current].clear();
        self.current -= 1;
    }

    pub fn pop_scopes<'b>(&'b mut self, index: usize) -> impl Iterator<Item = Scope<'a>> + 'b {
        self.current = index;
        self.scopes.drain(index + 1..)
    }

    pub fn append_scopes(&mut self, scopes: Vec<Scope<'a>>) {
        self.scopes.drain(self.current + 1..);
        self.current += scopes.len();
        self.scopes.extend(scopes);
    }

    pub fn scopes(&self) -> impl DoubleEndedIterator<Item = &Scope<'a>> {
        let amount = self.scopes.len() - self.current - 1;
        self.scopes.iter().rev().skip(amount)
    }

    pub fn scopes_mut(&mut self) -> impl Iterator<Item = &mut Scope<'a>> {
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

    pub fn shadow(&mut self, name: &str, value: Value<'a>) -> Option<Value<'a>> {
        self.scopes[self.current].insert(name.into(), value)
    }

    pub fn get_ref(&self, mut name: &str) -> Option<&Value<'a>> {
        const GLOBAL_NS: &str = "global::";
        const SUPER_NS: &str = "super::";

        if name.starts_with(GLOBAL_NS) {
            name = &name[GLOBAL_NS.len()..];
            // Go up as many namespaces as possible
            self.scopes()
                .rev()
                .take_while(|scope| !scope.namespace)
                .filter_map(|scope| scope.get(name))
                .last()
        } else if name.starts_with(SUPER_NS) {
            let mut up = 0;
            while name.starts_with(SUPER_NS) {
                name = &name[SUPER_NS.len()..];
                up += 1;
            }

            for scope in self.scopes() {
                if up == 0 {
                    let val = scope.get(name);
                    if val.is_some() {
                        return val;
                    } else if scope.namespace {
                        break;
                    }
                } else if scope.namespace {
                    up -= 1;
                }
            }

            None
        } else {
            self.scopes().filter_map(|scope| scope.get(name)).next()
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Value<'a>> {
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

    pub fn remove_variable(&mut self, name: &str) -> Option<Value<'a>> {
        if name.starts_with("super::") || name.starts_with("global::") {
            // Cannot mutate outer namespace
            return None;
        }
        for scope in self.scopes_mut() {
            let exit = scope.namespace;
            if let val @ Some(_) = scope.remove(name) {
                return val;
            }
            if exit {
                break;
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
                    if let Value::Str(val) = val {
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

    pub fn get<T>(&self, name: &str) -> Option<T>
    where
        Self: GetVariable<T>,
    {
        GetVariable::<T>::get(self, name)
    }

    pub fn set<T: Into<Value<'a>>>(&mut self, name: &str, var: T) {
        let var = var.into();

        enum UpperAction {
            Remove,
            Shadow,
        }

        enum Action<'a, 'b> {
            Upper(UpperAction),
            Alias(&'a mut types::Alias),
            Str(&'a mut types::Str),
            Array(&'a mut types::Array),
            Function(&'a mut Function<'b>),
            HashMap(&'a mut types::HashMap<'b>),
        }

        macro_rules! handle_type {
            ($name:tt, $input:ty, $preferred:tt) => {
                fn $name<'a, 'b>(
                    name: &str,
                    var: &Value<'a>,
                    input: &'b mut $input,
                ) -> Option<Action<'b, 'a>> {
                    if !name.is_empty() {
                        match var {
                            Value::$preferred(var_value) => {
                                if var_value.is_empty() {
                                    Some(Action::Upper(UpperAction::Remove))
                                } else {
                                    Some(Action::$preferred(input))
                                }
                            }
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
        handle_type!(hashmap_action, types::HashMap<'a>, HashMap);
        handle_type!(function_action, Function<'a>, Function);

        let upper_action = {
            let action = match self.get_mut(&name) {
                Some(Value::Str(ref mut str_)) => string_action(name, &var, str_),
                Some(Value::Alias(ref mut alias)) => alias_action(name, &var, alias),
                Some(Value::Array(ref mut array)) => array_action(name, &var, array),
                Some(Value::HashMap(ref mut map)) => hashmap_action(name, &var, map),
                Some(Value::Function(ref mut func)) => function_action(name, &var, func),
                _ => Some(Action::Upper(UpperAction::Shadow)),
            };

            macro_rules! handle_action {
                ($value:ident, $variant:tt) => {{
                    if let Value::$variant(with) = var {
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
                    if let Value::Alias(alias) = possible_alias {
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
                    if let Value::Function(val) = val {
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
            let elements = swd.split('/').filter(|s| !s.is_empty()).collect::<Vec<&str>>();
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
        let home = self.get::<types::Str>("HOME").unwrap_or_else(|| "?".into());
        env::var("PWD").unwrap().replace(&*home, "~").into()
    }

    pub fn arrays(&self) -> impl Iterator<Item = (&types::Str, &types::Array)> {
        self.scopes
            .iter()
            .rev()
            .map(|map| {
                map.iter().filter_map(|(key, val)| {
                    if let Value::Array(val) = val {
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

pub trait GetVariable<T> {
    fn get(&self, name: &str) -> Option<T>;
}

impl<'a> GetVariable<types::Str> for Variables<'a> {
    fn get(&self, name: &str) -> Option<types::Str> {
        use crate::types::Str;

        match name {
            "MWD" => return Some(Str::from(Value::Str(self.get_minimal_directory()))),
            "SWD" => return Some(Str::from(Value::Str(self.get_simplified_directory()))),
            _ => (),
        }
        // If the parsed name contains the '::' pattern, then a namespace was
        // designated. Find it.
        match name.find("::").map(|pos| (&name[..pos], &name[pos + 2..])) {
            Some(("c", variable)) | Some(("color", variable)) => {
                Colors::collect(variable).into_string().map(|s| Str::from(Value::Str(s.into())))
            }
            Some(("x", variable)) | Some(("hex", variable)) => {
                match u8::from_str_radix(variable, 16) {
                    Ok(c) => Some(Str::from(Value::Str((c as char).to_string().into()))),
                    Err(why) => {
                        eprintln!("ion: hex parse error: {}: {}", variable, why);
                        None
                    }
                }
            }
            Some(("env", variable)) => {
                env::var(variable).map(Into::into).ok().map(|s| Str::from(Value::Str(s)))
            }
            Some(("super", _)) | Some(("global", _)) | None => {
                // Otherwise, it's just a simple variable name.
                match self.get_ref(name) {
                    Some(Value::Str(val)) => Some(Str::from(Value::Str(val.clone()))),
                    _ => env::var(name).ok().map(|s| Str::from(Value::Str(s.into()))),
                }
            }
            Some((..)) => {
                eprintln!("ion: unsupported namespace: '{}'", name);
                None
            }
        }
    }
}

macro_rules! get_var {
    ($types:ty, $variant:ident($inner:ident) => $ret:expr) => {
        impl<'a> GetVariable<$types> for Variables<'a> {
            fn get(&self, name: &str) -> Option<$types> {
                match self.get_ref(name) {
                    Some(Value::$variant($inner)) => {
                        Some(<$types>::from(Value::$variant($ret.clone())))
                    }
                    _ => None,
                }
            }
        }
    };
}

get_var!(types::Alias, Alias(alias) => (*alias));
get_var!(types::Array, Array(array) => array);
get_var!(types::HashMap<'a>, HashMap(hmap) => hmap);
get_var!(types::BTreeMap<'a>, BTreeMap(bmap) => bmap);
get_var!(Function<'a>, Function(func) => func);

#[cfg(test)]
mod trait_test;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{expand_string, Expander};
    use serial_test_derive::serial;

    struct VariableExpander<'a>(pub Variables<'a>);

    impl<'a> Expander for VariableExpander<'a> {
        fn string(&self, var: &str) -> Option<types::Str> { self.0.get::<types::Str>(var) }
    }

    #[test]
    fn undefined_variable_expands_to_empty_string() {
        let variables = Variables::default();
        let expanded = expand_string("$FOO", &VariableExpander(variables)).join("");
        assert_eq!("", &expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.set("FOO", "BAR");
        let expanded = expand_string("$FOO", &VariableExpander(variables)).join("");
        assert_eq!("BAR", &expanded);
    }

    #[test]
    #[serial]
    fn minimal_directory_var_should_compact_path() {
        let variables = Variables::default();
        env::set_var("PWD", "/var/log/nix");
        assert_eq!(
            types::Str::from("v/l/nix"),
            variables.get::<types::Str>("MWD").expect("no value returned"),
        );
    }

    #[test]
    #[serial]
    fn minimal_directory_var_shouldnt_compact_path() {
        let variables = Variables::default();
        env::set_var("PWD", "/var/log");
        assert_eq!(
            types::Str::from("/var/log"),
            variables.get::<types::Str>("MWD").expect("no value returned"),
        );
    }
}
