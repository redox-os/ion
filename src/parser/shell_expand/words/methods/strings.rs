use super::MethodArgs;
use super::super::Select;
use super::super::super::{expand_string, is_expression, slice, Expander};
use parser::assignments::is_array;
use regex::Regex;
use shell::plugins::methods::{self, MethodArguments, StringMethodPlugins};
use std::path::Path;
use sys;
use unicode_segmentation::UnicodeSegmentation;

lazy_static! {
    static ref STRING_METHODS: StringMethodPlugins = methods::collect();
}

fn unescape(input: &str) -> Result<String, &'static str> {
    let mut check = false;
    let mut out = String::with_capacity(input.len());
    let add_char = |out: &mut String, check: &mut bool, c| {
        out.push(c);
        *check = false;
    };
    for c in input.chars() {
        match c {
            '\\' if check => {
                add_char(&mut out, &mut check, c);
            }
            '\\' => check = true,
            '\'' if check => add_char(&mut out, &mut check, c),
            '\"' if check => add_char(&mut out, &mut check, c),
            'a' if check => add_char(&mut out, &mut check, '\u{0007}'),
            'b' if check => add_char(&mut out, &mut check, '\u{0008}'),
            'c' if check => {
                out = String::from("");
                break;
            }
            'e' if check => add_char(&mut out, &mut check, '\u{001B}'),
            'f' if check => add_char(&mut out, &mut check, '\u{000C}'),
            'n' if check => add_char(&mut out, &mut check, '\n'),
            'r' if check => add_char(&mut out, &mut check, '\r'),
            't' if check => add_char(&mut out, &mut check, '\t'),
            'v' if check => add_char(&mut out, &mut check, '\u{000B}'),
            _ if check => {
                out.push('\\');
                add_char(&mut out, &mut check, c);
            }
            c if c.is_ascii() => out.push(c),
            _ => return Err("ion: Invalid ASCII character"),
        }
    }
    Ok(out)
}
fn escape(input: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(input.len() * 2);
    for b in input.as_bytes() {
        match *b {
            0 => output.push_str("\\0"),
            7 => output.push_str("\\a"),
            8 => output.push_str("\\b"),
            9 => output.push_str("\\t"),
            10 => output.push_str("\\n"),
            11 => output.push_str("\\v"),
            12 => output.push_str("\\f"),
            13 => output.push_str("\\r"),
            27 => output.push_str("\\e"),
            n if n != 59 && n != 95
                && ((n >= 33 && n < 48) || (n >= 58 && n < 65) || (n >= 91 && n < 97)
                    || (n >= 123 && n < 127)) =>
            {
                output.push('\\');
                output.push(n as char);
            }
            n if n <= 127 => output.push(n as char),
            _ => return Err("ion: Invalid ASCII character"),
        }
    }
    Ok(output)
}

/// Represents a method that operates on and returns a string
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct StringMethod<'a> {
    /// Name of this method
    pub(crate) method: &'a str,
    /// Variable that this method will operator on. This is a bit of a misnomer
    /// as this can be an expression as well
    pub(crate) variable: &'a str,
    /// Pattern to use for certain methods
    pub(crate) pattern: &'a str,
    /// Selection to use to control the output of this method
    pub(crate) selection: Select,
}

impl<'a> StringMethod<'a> {
    pub(crate) fn handle<E: Expander>(&self, output: &mut String, expand: &E) {
        let variable = self.variable;
        let pattern = MethodArgs::new(self.pattern, expand);

        macro_rules! string_eval {
            ($variable:ident $method:tt) => {{
                let pattern = pattern.join(" ");
                let is_true = if let Some(value) = expand.variable($variable, false) {
                    value.$method(&pattern)
                } else if is_expression($variable) {
                    expand_string($variable, expand, false).join(" ").$method(&pattern)
                } else {
                    false
                };
                output.push_str(if is_true { "1" } else { "0" });
            }}
        }

        macro_rules! path_eval {
            ($method:tt) => {{
                if let Some(value) = expand.variable(variable, false) {
                    output.push_str(Path::new(&value).$method()
                        .and_then(|os_str| os_str.to_str()).unwrap_or(value.as_str()));
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(" ");
                    output.push_str(Path::new(&word).$method()
                        .and_then(|os_str| os_str.to_str()).unwrap_or(word.as_str()));
                }
            }}
        }

        macro_rules! string_case {
            ($method:tt) => {{
                if let Some(value) = expand.variable(variable, false) {
                    output.push_str(value.$method().as_str());
                } else if is_expression(variable) {
                    let word = expand_string(variable, expand, false).join(" ");
                    output.push_str(word.$method().as_str());
                }
            }}
        }

        macro_rules! get_var {
            () => {{
                if let Some(value) = expand.variable(variable, false) {
                    value
                } else {
                    expand_string(variable, expand, false).join(" ")
                }
            }}
        }

        match self.method {
            "ends_with" => string_eval!(variable ends_with),
            "contains" => string_eval!(variable contains),
            "starts_with" => string_eval!(variable starts_with),
            "basename" => path_eval!(file_name),
            "extension" => path_eval!(extension),
            "filename" => path_eval!(file_stem),
            "parent" => path_eval!(parent),
            "to_lowercase" => string_case!(to_lowercase),
            "to_uppercase" => string_case!(to_uppercase),
            "repeat" => match pattern.join(" ").parse::<usize>() {
                Ok(repeat) => output.push_str(&get_var!().repeat(repeat)),
                Err(_) => {
                    eprintln!("ion: value supplied to $repeat() is not a valid positive integer");
                }
            },
            "replace" => {
                let mut args = pattern.array();
                match (args.next(), args.next()) {
                    (Some(replace), Some(with)) => {
                        let res = &get_var!().replace(&replace, &with);
                        output.push_str(res);
                    }
                    _ => eprintln!("ion: replace: two arguments are required"),
                }
            }
            "replacen" => {
                let mut args = pattern.array();
                match (args.next(), args.next(), args.next()) {
                    (Some(replace), Some(with), Some(nth)) => if let Ok(nth) = nth.parse::<usize>()
                    {
                        let res = &get_var!().replacen(&replace, &with, nth);
                        output.push_str(res);
                    } else {
                        eprintln!("ion: replacen: third argument isn't a valid integer");
                    },
                    _ => eprintln!("ion: replacen: three arguments required"),
                }
            }
            "regex_replace" => {
                let mut args = pattern.array();
                match (args.next(), args.next()) {
                    (Some(replace), Some(with)) => match Regex::new(&replace) {
                        Ok(re) => {
                            let inp = &get_var!();
                            let res = re.replace_all(&inp, &with[..]);
                            output.push_str(&res);
                        }
                        Err(_) => eprintln!(
                            "ion: regex_replace: error in regular expression {}",
                            &replace
                        ),
                    },
                    _ => eprintln!("ion: regex_replace: two arguments required"),
                }
            }
            "join" => {
                let pattern = pattern.join(" ");
                if let Some(array) = expand.array(variable, Select::All) {
                    slice(output, array.join(&pattern), self.selection.clone());
                } else if is_expression(variable) {
                    slice(
                        output,
                        expand_string(variable, expand, false).join(&pattern),
                        self.selection.clone(),
                    );
                }
            }
            "len" => if variable.starts_with('@') || is_array(variable) {
                let expanded = expand_string(variable, expand, false);
                output.push_str(&expanded.len().to_string());
            } else if let Some(value) = expand.variable(variable, false) {
                let count = UnicodeSegmentation::graphemes(value.as_str(), true).count();
                output.push_str(&count.to_string());
            } else if is_expression(variable) {
                let word = expand_string(variable, expand, false).join(" ");
                let count = UnicodeSegmentation::graphemes(word.as_str(), true).count();
                output.push_str(&count.to_string());
            },
            "len_bytes" => if let Some(value) = expand.variable(variable, false) {
                output.push_str(&value.as_bytes().len().to_string());
            } else if is_expression(variable) {
                let word = expand_string(variable, expand, false).join(" ");
                output.push_str(&word.as_bytes().len().to_string());
            },
            "reverse" => if let Some(value) = expand.variable(variable, false) {
                let rev_graphs = UnicodeSegmentation::graphemes(value.as_str(), true).rev();
                output.push_str(rev_graphs.collect::<String>().as_str());
            } else if is_expression(variable) {
                let word = expand_string(variable, expand, false).join(" ");
                let rev_graphs = UnicodeSegmentation::graphemes(word.as_str(), true).rev();
                output.push_str(rev_graphs.collect::<String>().as_str());
            },
            "find" => {
                let out = if let Some(value) = expand.variable(variable, false) {
                    value.find(&pattern.join(" "))
                } else if is_expression(variable) {
                    expand_string(variable, expand, false).join(" ").find(&pattern.join(" "))
                } else {
                    None
                };
                output.push_str(&out.map(|i| i as isize).unwrap_or(-1).to_string());
            }
            "unescape" => {
                let out = if let Some(value) = expand.variable(variable, false) {
                    value
                } else if is_expression(variable) {
                    expand_string(variable, expand, false).join(" ")
                } else {
                    return;
                };
                match unescape(&out) {
                    Ok(out) => output.push_str(&out),
                    Err(msg) => eprintln!("{}", &msg),
                };
            }
            "escape" => {
                let word = if let Some(value) = expand.variable(variable, false) {
                    value
                } else if is_expression(variable) {
                    expand_string(variable, expand, false).join(" ")
                } else {
                    return;
                };
                match escape(&word) {
                    Ok(out) => output.push_str(&out),
                    Err(msg) => eprintln!("{}", &msg),
                };
            }
            method @ _ => {
                if sys::is_root() {
                    eprintln!("ion: root is not allowed to execute plugins");
                    return;
                }

                let pattern = pattern.array().collect::<Vec<_>>();
                let args = if variable.starts_with('@') || is_array(variable) {
                    MethodArguments::Array(
                        expand_string(variable, expand, false).into_vec(),
                        pattern,
                    )
                } else if let Some(value) = expand.variable(variable, false) {
                    MethodArguments::StringArg(value, pattern)
                } else if is_expression(variable) {
                    let expanded = expand_string(variable, expand, false);
                    match expanded.len() {
                        0 => MethodArguments::NoArgs,
                        1 => MethodArguments::StringArg(expanded[0].clone(), pattern),
                        _ => MethodArguments::Array(expanded.into_vec(), pattern),
                    }
                } else {
                    MethodArguments::NoArgs
                };

                match STRING_METHODS.execute(method, args) {
                    Ok(Some(string)) => output.push_str(&string),
                    Ok(None) => (),
                    Err(why) => eprintln!("ion: method plugin: {}", why),
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use types::Value;

    struct VariableExpander;

    impl Expander for VariableExpander {
        fn variable(&self, variable: &str, _: bool) -> Option<Value> {
            match variable {
                "FOO" => Some("FOOBAR".to_owned()),
                _ => None,
            }
        }
    }

    #[test]
    fn test_escape() {
        let line = " Mary   had\ta little  \n\t lamb\t";
        let output = escape(line).expect("error processing string");
        assert_eq!(output, " Mary   had\\ta little  \\n\\t lamb\\t");
    }

    #[test]
    fn test_unescape() {
        let line = " Mary   had\ta little  \n\t lamb\t";
        let output = unescape(line).expect("error processing string");
        assert_eq!(output, line);
    }

    #[test]
    fn test_ends_with_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "ends_with",
            variable:  "$FOO",
            pattern:   "\"BAR\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "1");
    }

    #[test]
    fn test_ends_with_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "ends_with",
            variable:  "$FOO",
            pattern:   "\"BA\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "0");
    }

    #[test]
    fn test_contains_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "contains",
            variable:  "$FOO",
            pattern:   "\"OBA\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "1");
    }

    #[test]
    fn test_contains_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "contains",
            variable:  "$FOO",
            pattern:   "\"OBI\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "0");
    }

    #[test]
    fn test_starts_with_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "starts_with",
            variable:  "$FOO",
            pattern:   "\"FOO\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "1");
    }

    #[test]
    fn test_starts_with_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "starts_with",
            variable:  "$FOO",
            pattern:   "\"OO\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "0");
    }

    #[test]
    fn test_basename() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "basename",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "file.txt");
    }

    #[test]
    fn test_extension() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "extension",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "txt");
    }

    #[test]
    fn test_filename() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "filename",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "file");
    }

    #[test]
    fn test_parent() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "parent",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "/home/redox");
    }

    #[test]
    fn test_to_lowercase() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "to_lowercase",
            variable:  "\"Ford Prefect\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "ford prefect");
    }

    #[test]
    fn test_to_uppercase() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "to_uppercase",
            variable:  "\"Ford Prefect\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FORD PREFECT");
    }

    #[test]
    fn test_repeat_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "repeat",
            variable:  "$FOO",
            pattern:   "2",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FOOBARFOOBAR");
    }

    #[test]
    fn test_repeat_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "repeat",
            variable:  "$FOO",
            pattern:   "-2",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "");
    }

    #[test]
    fn test_replace_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "replace",
            variable:  "$FOO",
            pattern:   "[\"FOO\" \"BAR\"]",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "BARBAR");
    }

    #[test]
    fn test_replace_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "replace",
            variable:  "$FOO",
            pattern:   "[]",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "");
    }

    #[test]
    fn test_replacen_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "replacen",
            variable:  "\"FOO$FOO\"",
            pattern:   "[\"FOO\" \"BAR\" 1]",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "BARFOOBAR");
    }

    #[test]
    fn test_replacen_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "replacen",
            variable:  "$FOO",
            pattern:   "[]",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "");
    }

    #[test]
    fn test_join_with_string() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "join",
            variable:  "[\"FOO\" \"BAR\"]",
            pattern:   "\" \"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FOO BAR");
    }

    #[test]
    fn test_join_with_array() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "join",
            variable:  "[\"FOO\" \"BAR\"]",
            pattern:   "[\"-\" \"-\"]",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FOO- -BAR");
    }

    #[test]
    fn test_len_with_array() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "len",
            variable:  "[\"1\"]",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "1");
    }

    #[test]
    fn test_len_with_string() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "len",
            variable:  "\"FOO\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "3");
    }

    #[test]
    fn test_len_with_variable() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "len",
            variable:  "$FOO",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "6");
    }

    #[test]
    fn test_len_bytes_with_variable() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "len_bytes",
            variable:  "$FOO",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "6");
    }

    #[test]
    fn test_len_bytes_with_string() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "len_bytes",
            variable:  "\"oh là là\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "10");
    }

    #[test]
    fn test_reverse_with_variable() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "reverse",
            variable:  "$FOO",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "RABOOF");
    }

    #[test]
    fn test_reverse_with_string() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "reverse",
            variable:  "\"FOOBAR\"",
            pattern:   "",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "RABOOF");
    }

    #[test]
    fn test_find_succeeding() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "find",
            variable:  "$FOO",
            pattern:   "\"O\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "1");
    }

    #[test]
    fn test_find_failing() {
        let mut output = String::new();
        let method = StringMethod {
            method:    "find",
            variable:  "$FOO",
            pattern:   "\"L\"",
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "-1");
    }
}
