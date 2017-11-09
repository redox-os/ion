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

/// Represents a method that operates on and returns a string
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct StringMethod<'a> {
    /// Name of this method: currently `join`, `len`, and `len_bytes` are the
    /// supported methods
    pub(crate) method: &'a str,
    /// Variable that this method will operator on. This is a bit of a misnomer
    /// as this can be an expression as well
    pub(crate) variable: &'a str,
    /// Pattern to use for certain methods: currently `join` makes use of a
    /// pattern
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
                    eprintln!("ion: value supplied to $repeat() is not a valid number");
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
                output.push_str(&out.unwrap_or(0).to_string());
            },
            "unescape" => {
                fn unescape(input: String) -> String {
                    let mut check = false;
                    let mut out = String::with_capacity(input.len());
                    for c in input.chars() {
                        match c {
                            '\\' if check => {
                                out.push(c);
                                check = false;
                            }
                            '\\' => check = true,
                            '\'' if check => {
                                out.push(c);
                                check = false;
                            }
                            '\"' if check => {
                                out.push(c);
                                check = false;
                            }
                            'a' if check => {
                                out.push('\u{0007}');
                                check = false;
                            }
                            'b' if check => {
                                out.push('\u{0008}');
                                check = false;
                            }
                            'c' if check => {
                                out = String::from("");
                                break;
                            }
                            'e' if check => {
                                out.push('\u{001B}');
                                check = false;
                            }
                            'f' if check => {
                                out.push('\u{000C}');
                                check = false;
                            }
                            'n' if check => {
                                out.push('\n');
                                check = false;
                            }
                            'r' if check => {
                                out.push('\r');
                                check = false;
                            }
                            't' if check => {
                                out.push('\t');
                                check = false;
                            }
                            'v' if check => {
                                out.push('\u{000B}');
                                check = false;
                            }
                            _ if check => {
                                out.push('\\');
                                out.push(c);
                                check = false;
                            }
                            _ => { out.push(c); }
                        }
                    }
                    out
                }
                if let Some(value) = expand.variable(variable, false) {
                    output.push_str(&unescape(value));
                } else if is_expression(variable) {
                    output.push_str(&unescape(expand_string(variable, expand, false).join(" ")));
                };
            },
            "escape" => {
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
                            n if n != 59 && n != 95 &&
                                ((n >= 33 && n < 48) ||
                                 (n >= 58 && n < 65) ||
                                 (n >= 91 && n < 97) ||
                                 (n >= 123 && n < 127)) => {
                                output.push('\\');
                                output.push(n as char);
                            },
                            n if n <= 127 => output.push(n as char),
                            _ => return Err("ion: Invalid ASCII character"),
                        }
                    }
                    Ok(output)
                }
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
            },
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
