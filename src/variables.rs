use std::collections::BTreeMap;
use std::env;

use liner::Context;

use super::peg::{Pipeline, Job};
use super::status::{SUCCESS, FAILURE};
use super::directory_stack::DirectoryStack;

pub struct Variables {
    variables: BTreeMap<String, String>,
}

impl Default for Variables {
    fn default() -> Variables {
        Variables { variables: BTreeMap::new() }
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

    pub fn let_<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        match Variables::parse_assignment(args) {
            (Some(key), Some(value)) => {
                if !Variables::is_valid_variable_name(&key) {
                    println!("Invalid variable name");
                    return FAILURE;
                }
                self.variables.insert(key.to_string(), value.to_string());
            },
            (Some(_), None) => {
                println!("Please provide a value for the variable");
                return FAILURE;
            },
            _ => {
                for (key, value) in &self.variables {
                    println!("{}={}", key, value);
                }
            }
        }
        SUCCESS
    }

    pub fn drop_variable<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        let args = args.into_iter().collect::<Vec<I::Item>>();
        if args.len() <= 1 {
            println!("You must specify a variable name");
            return FAILURE;
        }
        for variable in args.iter().skip(1) {
            if let None = self.unset_var(variable.as_ref()) {
                println!("Undefined variable: {}", variable.as_ref());
                return FAILURE;
            }
        }
        SUCCESS
    }

    pub fn set_var(&mut self, name: &str, value: &str) {
        if !name.is_empty() {
            if value.is_empty() {
                self.variables.remove(&name.to_string());
            } else {
                self.variables.insert(name.to_string(), value.to_string());
            }
        }
    }

    pub fn get_var(&self, name: &str) -> Option<String> {
        self.variables.get(name).cloned().or(env::var(name).ok())
    }

    pub fn get_var_or_empty(&self, name: &str) -> String {
        self.get_var(name).unwrap_or_default()
    }

    pub fn unset_var(&mut self, name: &str) -> Option<String> {
        self.variables.remove(name)
    }

    fn parse_assignment<I: IntoIterator>(args: I) -> (Option<String>, Option<String>)
        where I::Item: AsRef<str>
    {
        let args = args.into_iter();
        let string: String = args.skip(1).fold(String::new(), |string, x| string + x.as_ref());
        let mut split = string.split('=');
        (split.next().and_then(|x| if x == "" { None } else { Some(x.to_owned()) }), split.next().and_then(|x| Some(x.to_owned())))
    }

    pub fn export_variable<I: IntoIterator>(&mut self, args: I) -> i32
        where I::Item: AsRef<str>
    {
        match Variables::parse_assignment(args) {
            (Some(key), Some(value)) => {
                if !Variables::is_valid_variable_name(&key) {
                    println!("Invalid variable name");
                    return FAILURE;
                }
                env::set_var(key, value);
            },
            (Some(key), None) => {
                if let Some(local_value) = self.get_var(&key) {
                    env::set_var(key, local_value);
                } else {
                    println!("Unknown variable: {}", &key);
                    return FAILURE;
                }
            },
            _ => {
                println!("Usage: export KEY=VALUE");
                return FAILURE;
            }
        }
        SUCCESS
    }

    pub fn expand_pipeline(&self, pipeline: &Pipeline, dir_stack: &DirectoryStack) -> Pipeline {
        // TODO don't copy everything
        // TODO ugh, I made it worse
        Pipeline::new(pipeline.jobs.iter().map(|job| self.expand_job(job, dir_stack)).collect(),
                      pipeline.stdin.clone(),
                      pipeline.stdout.clone())
    }

    pub fn expand_job(&self, job: &Job, dir_stack: &DirectoryStack) -> Job {
        // TODO don't copy everything
        Job::new(job.args
                     .iter()
                     .map(|original: &String| self.expand_string(original, dir_stack))
                     .collect(),
                 job.background)
    }

    pub fn is_valid_variable_character(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '?'
    }

    pub fn is_valid_variable_name(name: &str) -> bool {
        name.chars().all(Variables::is_valid_variable_character)
    }

    pub fn tilde_expansion(&self, word: String, dir_stack: &DirectoryStack) -> String {
        // If the word doesn't start with ~, just return it to avoid allocating an iterator
        if word.starts_with('~') {
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
                        return home.to_string_lossy().to_string() + remainder;
                    }
                }
                "+" => {
                    if let Some(pwd) = self.get_var("PWD") {
                        return pwd.to_string() + remainder;
                    } else if let Ok(pwd) = env::current_dir() {
                        return pwd.to_string_lossy().to_string() + remainder;
                    }
                }
                "-" => {
                    if let Some(oldpwd) = self.get_var("OLDPWD") {
                        return oldpwd.to_string() + remainder;
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

                    if let Ok(num) = tilde_num.parse::<usize>() {
                        let res = if neg {
                            dir_stack.dir_from_top(num)
                        } else {
                            dir_stack.dir_from_bottom(num)
                        };

                        if let Some(path) = res {
                            return path.to_str().unwrap().to_string();
                        }
                    }
                }
            }
        }

        word
    }

    fn brace_expand(&self, input: String) -> String {
        let mut ignore_next = false;
        let mut brace_found = false;
        let mut variable_expansion_found;
        let mut infix_is_variable = false;
        let mut output = input.clone();

        loop {
            variable_expansion_found = false;
            let temp = output.clone();
            let mut char_iter = temp.chars();

            // Prefix Phase
            let mut prefix = String::new();
            while let Some(character) = char_iter.next() {
                match character {
                    '$'  if !ignore_next => { infix_is_variable = true; break },
                    '{'  if !ignore_next => {
                        brace_found = true;
                        break
                    },
                    '\\' if !ignore_next => ignore_next = true,
                    '\\'                 => { prefix.push(character); ignore_next = false; },
                    _ if ignore_next     => { prefix.push(character); ignore_next = false; },
                    _                    => prefix.push(character),
                }
            }

            // Infix Phase
            let mut infixes = Vec::new();
            if infix_is_variable {
                let mut colon_found = false;
                let mut variable = String::new();
                if let Some(character) = char_iter.next() {
                    if character == '{' {
                        variable_expansion_found = true;
                        while let Some(character) = char_iter.next() {
                            if character == '}' { break }
                            variable.push(character);
                        }
                    } else {
                        variable.push(character);
                        while let Some(character) = char_iter.next() {
                            if character == ':' {
                                variable_expansion_found = true;
                                colon_found = true;
                                break
                            } else {
                                variable.push(character);
                            }
                        }
                    }
                }

                if colon_found {
                    infixes.push(self.get_var(&variable).map_or(String::from(":"), |value| value + ":"));
                } else {
                    infixes.push(self.get_var(&variable).map_or(String::default(), |value| value));
                }
            } else if !brace_found {
                return input;
            } else {
                brace_found = false;
                ignore_next = false;
                let mut current = String::new();
                while let Some(character) = char_iter.next() {
                    match character {
                        '}'  if !ignore_next => {
                            infixes.push(current.clone());
                            current.clear();
                            brace_found = true;
                            break
                        },
                        '\\' if !ignore_next => ignore_next = true,
                        '\\'                 => { current.push(character); ignore_next = false; },
                        ',' if !ignore_next  => { infixes.push(current.clone()); current.clear(); },
                        _ if ignore_next     => { current.push(character); ignore_next = false; },
                        _                    => current.push(character),
                    }
                }

                if !brace_found { return input; }
            }

            // Suffix Phase
            ignore_next = false;
            let mut suffix = String::new();
            for character in char_iter {
                match character {
                    '\\' if !ignore_next => ignore_next = true,
                    _    if !ignore_next => suffix.push(character),
                    _                    => { suffix.push(character); ignore_next = false; },
                }
            }

            // Combine
            output = infixes.iter().map(|infix| prefix.clone() + infix + &suffix)
                .collect::<Vec<String>>().join(" ");

            if !variable_expansion_found { break }
        }
        output
    }

    pub fn expand_string<'a>(&'a self, original: &'a str, dir_stack: &DirectoryStack) -> String {
        let mut output = String::new();
        let mut current = String::new();
        for character in original.chars() {
            match character {
                ' ' if current.is_empty() => output.push(' '),
                ' ' => {
                    output.push_str(&self.brace_expand(self.tilde_expansion(current.clone(), dir_stack)));
                    output.push(' ');
                    current.clear();
                },
                _ => current.push(character)
            }
        }

        if !current.is_empty() {
            output.push_str(&self.brace_expand(self.tilde_expansion(current, dir_stack)));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use status::{FAILURE, SUCCESS};
    use directory_stack::DirectoryStack;

    fn new_dir_stack() -> DirectoryStack {
        DirectoryStack::new().unwrap()
    }

    #[test]
    fn undefined_variable_expands_to_empty_string() {
        let variables = Variables::default();
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("", &expanded);
    }

    #[test]
    fn let_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn set_var_and_expand_a_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("BAR", &expanded);
    }

    #[test]
    fn let_fails_if_no_value() {
        let mut variables = Variables::default();
        let return_status = variables.let_(vec!["let", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn expand_several_variables() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        variables.let_(vec!["let", "X", "=", "Y"]);
        let expanded = variables.expand_string("variables: $FOO $X", &new_dir_stack());
        assert_eq!("variables: BAR Y", &expanded);
    }

    #[test]
    fn expand_long_braces() {
        let variables = Variables::default();
        let line = "The pro{digal,grammer,cessed,totype,cedures,ficiently,ving,spective,jections}";
        let expected = "The prodigal programmer processed prototype procedures proficiently proving prospective projections";
        let expanded = variables.expand_string(line, &new_dir_stack());
        assert_eq!(expected, &expanded);
    }

    #[test]
    fn expand_several_braces() {
        let variables = Variables::default();
        let line = "The {barb,veget}arian eat{ers,ing} appl{esauce,ied} am{ple,ounts} of eff{ort,ectively}";
        let expected = "The barbarian vegetarian eaters eating applesauce applied ample amounts of effort effectively";
        let expanded = variables.expand_string(line, &new_dir_stack());
        assert_eq!(expected, &expanded);
    }

    #[test]
    fn expand_variable_braces() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "BAR"]);
        let expanded = variables.expand_string("FOO$FOO", &new_dir_stack());
        assert_eq!("FOOBAR", &expanded);
        let expanded = variables.expand_string(" FOO${FOO} ", &new_dir_stack());
        assert_eq!(" FOOBAR ", &expanded);
    }

    #[test]
    fn expand_variables_with_colons() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "FOO", "=", "FOO"]);
        variables.let_(vec!["let", "BAR", "=", "BAR"]);
        let expanded = variables.expand_string("$FOO:$BAR", &new_dir_stack());
        assert_eq!("FOO:BAR", &expanded);
    }

    #[test]
    fn expand_multiple_variables() {
        let mut variables = Variables::default();
        variables.let_(vec!["let", "A", "=", "test"]);
        variables.let_(vec!["let", "B", "=", "ing"]);
        variables.let_(vec!["let", "C", "=", "1 2 3"]);
        let expanded = variables.expand_string("${A}${B}...${C}", &new_dir_stack());
        assert_eq!("testing...1 2 3", &expanded);
    }

    #[test]
    fn escape_with_backslash() {
        let variables = Variables::default();
        let expanded = variables.expand_string("\\$FOO", &new_dir_stack());
        assert_eq!("\\$FOO", &expanded);
    }

    #[test]
    fn let_checks_variable_name() {
        let mut variables = Variables::default();
        let return_status = variables.let_(vec!["let", ",;!:", "=", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_deletes_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let return_status = variables.drop_variable(vec!["drop", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = variables.expand_string("$FOO", &new_dir_stack());
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = variables.drop_variable(vec!["drop"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut variables = Variables::default();
        let return_status = variables.drop_variable(vec!["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }
}
