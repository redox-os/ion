use super::{Pattern, Select, SelectWithSize};
use super::pattern::unescape;
use super::super::Index;
use super::super::super::{expand_string, Expander};
use super::super::super::is_expression;
use smallstring::SmallString;
use std::char;
use std::io::{self, Write};
use types::Array;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ArrayMethod<'a> {
    pub(crate) method:    &'a str,
    pub(crate) variable:  &'a str,
    pub(crate) pattern:   Pattern<'a>,
    pub(crate) selection: Select,
}

impl<'a> ArrayMethod<'a> {
    pub(crate) fn handle<E: Expander>(&self, current: &mut String, expand_func: &E) {
        match self.method {
            "split" => {
                let variable = if let Some(variable) = expand_func.variable(self.variable, false) {
                    variable
                } else if is_expression(self.variable) {
                    expand_string(self.variable, expand_func, false).join(" ")
                } else {
                    return;
                };
                match (&self.pattern, self.selection.clone()) {
                    (&Pattern::StringPattern(pattern), Select::All) => current.push_str(&variable
                        .split(&unescape(expand_string(pattern, expand_func, false).join(" ")))
                        .collect::<Vec<&str>>()
                        .join(" ")),
                    (&Pattern::Whitespace, Select::All) => current.push_str(&variable
                        .split(char::is_whitespace)
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<&str>>()
                        .join(" ")),
                    (_, Select::None) => (),
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Forward(id))) => {
                        current.push_str(
                            variable
                                .split(
                                    &unescape(expand_string(pattern, expand_func, false).join(" ")),
                                )
                                .nth(id)
                                .unwrap_or_default(),
                        )
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Forward(id))) => current.push_str(
                        variable
                            .split(char::is_whitespace)
                            .filter(|x| !x.is_empty())
                            .nth(id)
                            .unwrap_or_default(),
                    ),
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Backward(id))) => {
                        current.push_str(
                            variable
                                .rsplit(
                                    &unescape(expand_string(pattern, expand_func, false).join(" ")),
                                )
                                .nth(id)
                                .unwrap_or_default(),
                        )
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Backward(id))) => current
                        .push_str(
                            variable
                                .rsplit(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .nth(id)
                                .unwrap_or_default(),
                        ),
                    (&Pattern::StringPattern(pattern), Select::Range(range)) => {
                        let expansion = unescape(
                            unescape(expand_string(pattern, expand_func, false).join(" ")),
                        );
                        let iter = variable.split(&expansion);
                        if let Some((start, length)) = range.bounds(iter.clone().count()) {
                            let range = iter.skip(start).take(length).collect::<Vec<_>>().join(" ");
                            current.push_str(&range)
                        }
                    }
                    (&Pattern::Whitespace, Select::Range(range)) => {
                        let len =
                            variable.split(char::is_whitespace).filter(|x| !x.is_empty()).count();
                        if let Some((start, length)) = range.bounds(len) {
                            let range = variable
                                .split(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .skip(start)
                                .take(length)
                                .collect::<Vec<&str>>()
                                .join(" ");
                            current.push_str(&range);
                        }
                    }
                    (_, Select::Key(_)) => (),
                }
            }
            _ => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: invalid array method: {}", self.method);
            }
        }
    }

    pub(crate) fn handle_as_array<E: Expander>(&self, expand_func: &E) -> Array {
        macro_rules! resolve_var {
            () => {
                if let Some(variable) = expand_func.variable(self.variable, false) {
                    variable
                } else if is_expression(self.variable) {
                    expand_string(self.variable, expand_func, false).join(" ")
                } else {
                    "".into()
                }
            }
        }

        match self.method {
            "split" => {
                let variable = resolve_var!();
                return match (&self.pattern, self.selection.clone()) {
                    (_, Select::None) => Some("".into()).into_iter().collect(),
                    (&Pattern::StringPattern(pattern), Select::All) => variable
                        .split(&unescape(expand_string(pattern, expand_func, false).join(" ")))
                        .map(From::from)
                        .collect(),
                    (&Pattern::Whitespace, Select::All) => variable
                        .split(char::is_whitespace)
                        .filter(|x| !x.is_empty())
                        .map(From::from)
                        .collect(),
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Forward(id))) => {
                        variable
                            .split(&unescape(expand_string(pattern, expand_func, false).join(" ")))
                            .nth(id)
                            .map(From::from)
                            .into_iter()
                            .collect()
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Forward(id))) => variable
                        .split(char::is_whitespace)
                        .filter(|x| !x.is_empty())
                        .nth(id)
                        .map(From::from)
                        .into_iter()
                        .collect(),
                    (&Pattern::StringPattern(pattern), Select::Index(Index::Backward(id))) => {
                        variable
                            .rsplit(&unescape(expand_string(pattern, expand_func, false).join(" ")))
                            .nth(id)
                            .map(From::from)
                            .into_iter()
                            .collect()
                    }
                    (&Pattern::Whitespace, Select::Index(Index::Backward(id))) => variable
                        .rsplit(char::is_whitespace)
                        .filter(|x| !x.is_empty())
                        .nth(id)
                        .map(From::from)
                        .into_iter()
                        .collect(),
                    (&Pattern::StringPattern(pattern), Select::Range(range)) => {
                        let expansion =
                            unescape(expand_string(pattern, expand_func, false).join(" "));
                        let iter = variable.split(&expansion);
                        if let Some((start, length)) = range.bounds(iter.clone().count()) {
                            iter.skip(start).take(length).map(From::from).collect()
                        } else {
                            Array::new()
                        }
                    }
                    (&Pattern::Whitespace, Select::Range(range)) => {
                        let len =
                            variable.split(char::is_whitespace).filter(|x| !x.is_empty()).count();
                        if let Some((start, length)) = range.bounds(len) {
                            variable
                                .split(char::is_whitespace)
                                .filter(|x| !x.is_empty())
                                .skip(start)
                                .take(length)
                                .map(From::from)
                                .collect()
                        } else {
                            Array::new()
                        }
                    }
                    (_, Select::Key(_)) => Some("".into()).into_iter().collect(),
                };
            }
            "split_at" => {
                let variable = resolve_var!();
                match self.pattern {
                    Pattern::StringPattern(string) => if let Ok(value) =
                        expand_string(string, expand_func, false).join(" ").parse::<usize>()
                    {
                        if value < variable.len() {
                            let (l, r) = variable.split_at(value);
                            return array![SmallString::from(l), SmallString::from(r)];
                        }
                        eprintln!("ion: split_at: value is out of bounds");
                    } else {
                        eprintln!("ion: split_at: requires a valid number as an argument");
                    },
                    Pattern::Whitespace => {
                        eprintln!("ion: split_at: requires an argument");
                    }
                }
            }
            "graphemes" => {
                let variable = resolve_var!();
                let graphemes = UnicodeSegmentation::graphemes(variable.as_str(), true);
                let len = graphemes.clone().count();
                return graphemes.map(From::from).select(self.selection.clone(), len);
            }
            "bytes" => {
                let variable = resolve_var!();
                let len = variable.as_bytes().len();
                return variable.bytes().map(|b| b.to_string()).select(self.selection.clone(), len);
            }
            "chars" => {
                let variable = resolve_var!();
                let len = variable.chars().count();
                return variable.chars().map(|c| c.to_string()).select(self.selection.clone(), len);
            }
            _ => {
                let stderr = io::stderr();
                let mut stderr = stderr.lock();
                let _ = writeln!(stderr, "ion: invalid array method: {}", self.method);
            }
        }

        array![]
    }
}
