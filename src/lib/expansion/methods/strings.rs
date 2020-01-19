use super::{super::Select, MethodArgs, MethodError};
use crate::{
    assignments::is_array,
    expansion::{is_expression, Error, Expander, ExpanderInternal, Result},
    types,
};
use regex::Regex;
use std::path::Path;
use unicode_segmentation::UnicodeSegmentation;

pub fn unescape(input: &str) -> types::Str {
    let mut check = false;
    // types::Str cannot be created with a capacity of 0 without causing a panic
    let len = if input.is_empty() { 1 } else { input.len() };
    let mut out = types::Str::with_capacity(len);
    let add_char = |out: &mut types::Str, check: &mut bool, c| {
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
                out = types::Str::from("");
                break;
            }
            'e' if check => add_char(&mut out, &mut check, '\u{001B}'),
            'f' if check => add_char(&mut out, &mut check, '\u{000C}'),
            'n' if check => add_char(&mut out, &mut check, '\n'),
            'r' if check => add_char(&mut out, &mut check, '\r'),
            't' if check => add_char(&mut out, &mut check, '\t'),
            'v' if check => add_char(&mut out, &mut check, '\u{000B}'),
            ' ' if check => add_char(&mut out, &mut check, c),
            _ if check => {
                out.push('\\');
                add_char(&mut out, &mut check, c);
            }
            c => out.push(c),
        }
    }
    out
}
fn escape(input: &str) -> String {
    let mut output = String::with_capacity(input.len() * 2);
    for b in input.chars() {
        match b as u8 {
            0 => output.push_str("\\0"),
            7 => output.push_str("\\a"),
            8 => output.push_str("\\b"),
            9 => output.push_str("\\t"),
            10 => output.push_str("\\n"),
            11 => output.push_str("\\v"),
            12 => output.push_str("\\f"),
            13 => output.push_str("\\r"),
            27 => output.push_str("\\e"),
            n if n != 59
                && n != 95
                && ((n >= 33 && n < 48)
                    || (n >= 58 && n < 65)
                    || (n >= 91 && n < 97)
                    || (n >= 123 && n < 127)) =>
            {
                output.push('\\');
                output.push(n as char);
            }
            _ => output.push(b),
        }
    }
    output
}

/// Represents a method that operates on and returns a string
#[derive(Debug, PartialEq, Clone)]
pub struct StringMethod<'a> {
    /// Name of this method
    pub method:    &'a str,
    /// Variable that this method will operator on. This is a bit of a misnomer
    /// as this can be an expression as well
    pub variable:  &'a str,
    /// Pattern to use for certain methods
    pub pattern:   &'a str,
    /// Selection to use to control the output of this method
    pub selection: Option<&'a str>,
}

impl<'a> StringMethod<'a> {
    pub fn handle<E: Expander>(
        &self,
        output: &mut types::Str,
        expand: &mut E,
    ) -> Result<(), E::Error> {
        let variable = self.variable;

        macro_rules! path_eval {
            ($method:tt) => {{
                match expand.string(variable) {
                    Ok(value) => output.push_str(
                        Path::new(&*value)
                            .$method()
                            .and_then(|os_str| os_str.to_str())
                            .unwrap_or(value.as_str()),
                    ),
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        let word = expand.expand_string(variable)?.join(" ");
                        output.push_str(
                            Path::new(&word)
                                .$method()
                                .and_then(|os_str| os_str.to_str())
                                .unwrap_or(word.as_str()),
                        );
                    }
                    Err(why) => return Err(why),
                }
            }};
        }

        macro_rules! string_case {
            ($method:tt) => {{
                match expand.string(variable) {
                    Ok(value) => output.push_str(value.$method().as_str()),
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        let word = expand.expand_string(variable)?.join(" ");
                        output.push_str(word.$method().as_str());
                    }
                    Err(why) => return Err(why),
                }
            }};
        }

        macro_rules! get_var {
            () => {{
                let string = expand.string(variable);
                match string {
                    Ok(value) => value,
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        types::Str::from(expand.expand_string(variable)?.join(" "))
                    }
                    Err(why) => return Err(why),
                }
            }};
        }

        match self.method {
            "basename" => path_eval!(file_name),
            "extension" => path_eval!(extension),
            "filename" => path_eval!(file_stem),
            "parent" => path_eval!(parent),
            "to_lowercase" => string_case!(to_lowercase),
            "to_uppercase" => string_case!(to_uppercase),
            "trim" => output.push_str(get_var!().trim()),
            "trim_end" => output.push_str(get_var!().trim_end()),
            "trim_start" => output.push_str(get_var!().trim_start()),
            "repeat" => match MethodArgs::new(self.pattern, expand).join(" ")?.parse::<usize>() {
                Ok(repeat) => output.push_str(&get_var!().repeat(repeat)),
                Err(_) => {
                    return Err(MethodError::WrongArgument(
                        "repeat",
                        "argument is not a valid positive integer",
                    )
                    .into())
                }
            },
            "replace" => {
                let params = {
                    let mut args = MethodArgs::new(self.pattern, expand);
                    let mut args = args.array();
                    (args.next(), args.next())
                };
                match params {
                    (Some(replace), Some(with)) => {
                        output.push_str(&get_var!().replace(replace.as_str(), &with));
                    }
                    _ => {
                        return Err(MethodError::WrongArgument(
                            "replace",
                            "two arguments are required",
                        )
                        .into())
                    }
                }
            }
            "replacen" => {
                let params = {
                    let mut args = MethodArgs::new(self.pattern, expand);
                    let mut args = args.array();
                    (args.next(), args.next(), args.next())
                };
                match params {
                    (Some(replace), Some(with), Some(nth)) => {
                        if let Ok(nth) = nth.parse::<usize>() {
                            output.push_str(&get_var!().replacen(replace.as_str(), &with, nth));
                        } else {
                            return Err(MethodError::WrongArgument(
                                "replacen",
                                "third argument isn't a valid integer",
                            )
                            .into());
                        }
                    }
                    _ => {
                        return Err(MethodError::WrongArgument(
                            "replacen",
                            "three arguments required",
                        )
                        .into())
                    }
                }
            }
            "regex_replace" => {
                let params = {
                    let mut args = MethodArgs::new(self.pattern, expand);
                    let mut args = args.array();
                    (args.next(), args.next())
                };
                match params {
                    (Some(replace), Some(with)) => match Regex::new(&replace) {
                        Ok(re) => output.push_str(&re.replace_all(&get_var!(), &with[..])),
                        Err(why) => {
                            return Err(MethodError::InvalidRegex(replace.to_string(), why).into())
                        }
                    },
                    _ => {
                        return Err(MethodError::WrongArgument(
                            "regex_replace",
                            "two arguments required",
                        )
                        .into())
                    }
                }
            }
            "join" => {
                let pattern = MethodArgs::new(self.pattern, expand).join(" ")?;
                match expand.array(variable, &Select::All) {
                    Ok(array) => expand.slice(output, array.join(&pattern), &self.selection)?,
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        let expanded = expand.expand_string(variable)?.join(&pattern);
                        expand.slice(output, expanded, &self.selection)?
                    }
                    Err(why) => return Err(why),
                }
            }
            "len" => {
                if variable.starts_with('@') || is_array(variable) {
                    let expanded = expand.expand_string(variable)?;
                    output.push_str(&expanded.len().to_string());
                } else {
                    match expand.string(variable) {
                        Ok(value) => {
                            let count =
                                UnicodeSegmentation::graphemes(value.as_str(), true).count();
                            output.push_str(&count.to_string());
                        }
                        Err(Error::VarNotFound) if is_expression(variable) => {
                            let word = expand.expand_string(variable)?.join(" ");
                            let count = UnicodeSegmentation::graphemes(word.as_str(), true).count();
                            output.push_str(&count.to_string());
                        }
                        Err(why) => return Err(why),
                    }
                }
            }
            "len_bytes" => match expand.string(variable) {
                Ok(value) => output.push_str(&value.as_bytes().len().to_string()),
                Err(Error::VarNotFound) if is_expression(variable) => {
                    let word = expand.expand_string(variable)?.join(" ");
                    output.push_str(&word.as_bytes().len().to_string());
                }
                Err(why) => return Err(why),
            },
            "reverse" => match expand.string(variable) {
                Ok(value) => {
                    let rev_graphs = UnicodeSegmentation::graphemes(value.as_str(), true).rev();
                    output.push_str(rev_graphs.collect::<String>().as_str());
                }
                Err(Error::VarNotFound) if is_expression(variable) => {
                    let word = expand.expand_string(variable)?.join(" ");
                    let rev_graphs = UnicodeSegmentation::graphemes(word.as_str(), true).rev();
                    output.push_str(rev_graphs.collect::<String>().as_str());
                }
                Err(why) => return Err(why),
            },
            "find" => {
                let pattern = MethodArgs::new(self.pattern, expand).join(" ")?;
                let out = match expand.string(variable) {
                    Ok(value) => value.find(pattern.as_str()),
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        expand.expand_string(variable)?.join(" ").find(pattern.as_str())
                    }
                    Err(why) => return Err(why),
                };
                output.push_str(&out.map_or(-1, |i| i as isize).to_string());
            }
            "unescape" => {
                let out = match expand.string(variable) {
                    Ok(value) => value,
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        expand.expand_string(variable)?.join(" ").into()
                    }
                    Err(why) => return Err(why),
                };
                output.push_str(&unescape(&out));
            }
            "escape" => {
                let word = match expand.string(variable) {
                    Ok(value) => value,
                    Err(Error::VarNotFound) if is_expression(variable) => {
                        expand.expand_string(variable)?.join(" ").into()
                    }
                    Err(why) => return Err(why),
                };
                output.push_str(&escape(&word));
            }
            "or" => {
                let first_str = match expand.string(variable) {
                    Ok(value) => value,
                    Err(Error::VarNotFound) if is_expression(variable) => expand
                        .expand_string(variable)
                        .map(|x| x.join(" ").into())
                        .unwrap_or_default(),
                    Err(why) => return Err(why),
                };

                if first_str.is_empty() {
                    // Note that these commas should probably not be here and that this
                    // is the wrong place to handle this
                    if let Some(elem) = MethodArgs::new(self.pattern, expand)
                        .array()
                        .find(|elem| elem != "" && elem != ",")
                    {
                        // If the separation commas are properly removed from the
                        // pattern, then the cleaning on the next 7 lines is unnecessary
                        if elem.ends_with(',') {
                            let comma_pos = elem.rfind(',').unwrap();
                            let (clean, _) = elem.split_at(comma_pos);
                            output.push_str(clean);
                        } else {
                            output.push_str(&elem);
                        }
                    }
                } else {
                    output.push_str(&first_str)
                };
            }
            _ => {
                return Err(
                    Error::from(MethodError::InvalidScalarMethod(self.method.to_string())).into()
                )
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{expansion::test::DummyExpander, types};

    #[test]
    fn test_escape() {
        let line = " Mary   had\ta little  \n\t lamb\tツ";
        let output = escape(line);
        assert_eq!(output, " Mary   had\\ta little  \\n\\t lamb\\tツ");
    }

    #[test]
    fn test_unescape() {
        let line = " Mary   had\ta little  \n\t lamb\tツ";
        let output = unescape(line);
        assert_eq!(output, line);
    }

    #[test]
    fn test_basename() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "basename",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "file.txt");
    }

    #[test]
    fn test_extension() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "extension",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "txt");
    }

    #[test]
    fn test_filename() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "filename",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "file");
    }

    #[test]
    fn test_parent() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "parent",
            variable:  "\"/home/redox/file.txt\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "/home/redox");
    }

    #[test]
    fn test_to_lowercase() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "to_lowercase",
            variable:  "\"Ford Prefect\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "ford prefect");
    }

    #[test]
    fn test_to_uppercase() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "to_uppercase",
            variable:  "\"Ford Prefect\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FORD PREFECT");
    }

    #[test]
    fn test_trim_with_string() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "trim",
            variable:  "\"  Foo Bar \"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "Foo Bar");
    }

    #[test]
    fn test_trim_with_variable() {
        let mut output = types::Str::new();
        let method =
            StringMethod { method: "trim", variable: "$BAZ", pattern: "", selection: None };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "BARBAZ");
    }

    #[test]
    fn test_trim_end_with_string() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "trim_end",
            variable:  "\"  Foo Bar \"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "  Foo Bar");
    }

    #[test]
    fn test_trim_end_with_variable() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "trim_end",
            variable:  "$BAZ",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "  BARBAZ");
    }

    #[test]
    fn test_trim_start_with_string() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "trim_start",
            variable:  "\"  Foo Bar \"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "Foo Bar ");
    }

    #[test]
    fn test_trim_start_with_variable() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "trim_start",
            variable:  "$BAZ",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "BARBAZ   ");
    }

    #[test]
    fn test_repeat_succeeding() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "repeat",
            variable:  "$FOO",
            pattern:   "2",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOOBARFOOBAR");
    }

    #[test]
    #[should_panic]
    fn test_repeat_failing() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "repeat",
            variable:  "$FOO",
            pattern:   "-2",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
    }

    #[test]
    fn test_replace_succeeding() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "replace",
            variable:  "$FOO",
            pattern:   "[\"FOO\" \"BAR\"]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "BARBAR");
    }

    #[test]
    #[should_panic]
    fn test_replace_failing() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "replace",
            variable:  "$FOO",
            pattern:   "[]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
    }

    #[test]
    fn test_replacen_succeeding() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "replacen",
            variable:  "\"FOO$FOO\"",
            pattern:   "[\"FOO\" \"BAR\" 1]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "BARFOOBAR");
    }

    #[test]
    #[should_panic]
    fn test_replacen_failing() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "replacen",
            variable:  "$FOO",
            pattern:   "[]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
    }

    #[test]
    fn test_regex_replace_succeeding() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "regex_replace",
            variable:  "$FOO",
            pattern:   "[\"^F\" \"f\"]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "fOOBAR");
    }

    #[test]
    fn test_regex_replace_failing() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "regex_replace",
            variable:  "$FOO",
            pattern:   "[\"^f\" \"F\"]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOOBAR");
    }

    #[test]
    fn test_join_with_string() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "join",
            variable:  "[\"FOO\" \"BAR\"]",
            pattern:   "\" \"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_join_with_array() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "join",
            variable:  "[\"FOO\" \"BAR\"]",
            pattern:   "[\"-\" \"-\"]",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOO- -BAR");
    }

    #[test]
    fn test_len_with_array() {
        let mut output = types::Str::new();
        let method =
            StringMethod { method: "len", variable: "[\"1\"]", pattern: "", selection: None };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "1");
    }

    #[test]
    fn test_len_with_string() {
        let mut output = types::Str::new();
        let method =
            StringMethod { method: "len", variable: "\"FOO\"", pattern: "", selection: None };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "3");
    }

    #[test]
    fn test_len_with_variable() {
        let mut output = types::Str::new();
        let method =
            StringMethod { method: "len", variable: "$FOO", pattern: "", selection: None };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "6");
    }

    #[test]
    fn test_len_bytes_with_variable() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "len_bytes",
            variable:  "$FOO",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "6");
    }

    #[test]
    fn test_len_bytes_with_string() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "len_bytes",
            variable:  "\"oh là là\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "10");
    }

    #[test]
    fn test_reverse_with_variable() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "reverse",
            variable:  "$FOO",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "RABOOF");
    }

    #[test]
    fn test_reverse_with_string() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "reverse",
            variable:  "\"FOOBAR\"",
            pattern:   "",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "RABOOF");
    }

    #[test]
    fn test_find_succeeding() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "find",
            variable:  "$FOO",
            pattern:   "\"O\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "1");
    }

    #[test]
    fn test_find_failing() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "find",
            variable:  "$FOO",
            pattern:   "\"L\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "-1");
    }

    #[test]
    fn test_or_undefined() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "or",
            variable:  "$NDIUKFBINCF",
            pattern:   "\"baz\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "baz");
    }

    #[test]
    fn test_or_empty() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "or",
            variable:  "$EMPTY",
            pattern:   "\"baz\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "baz");
    }

    #[test]
    fn test_or_defined() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "or",
            variable:  "$FOO",
            pattern:   "\"baz\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOOBAR");
    }

    #[test]
    fn test_or_three_args_second_arg_defined() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "or",
            variable:  "$EMPTY",
            pattern:   "\"bar\", \"baz\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "bar");
    }

    #[test]
    fn test_or_three_args_third_arg_defined() {
        let mut output = types::Str::new();
        let method = StringMethod {
            method:    "or",
            variable:  "$EMPTY",
            pattern:   "\"\", \"baz\"",
            selection: None,
        };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "baz");
    }

    #[test]
    fn test_or_no_pattern() {
        let mut output = types::Str::new();
        let method =
            StringMethod { method: "or", variable: "$FOO", pattern: "\"\"", selection: None };
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOOBAR");
    }
}
