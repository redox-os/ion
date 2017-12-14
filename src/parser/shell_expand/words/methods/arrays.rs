use super::Pattern;
use super::strings::unescape;
use super::super::{Index, Select, SelectWithSize};
use super::super::super::{expand_string, is_expression, Expander};
use smallstring::SmallString;
use std::char;
use types::Array;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ArrayMethod<'a> {
    pub(crate) method: &'a str,
    pub(crate) variable: &'a str,
    pub(crate) pattern: Pattern<'a>,
    pub(crate) selection: Select,
}

impl<'a> ArrayMethod<'a> {
    pub(crate) fn handle<E: Expander>(&self, current: &mut String, expand_func: &E) {
        let res = match self.method {
            "split" => self.split(expand_func).map(|r| r.join(" ")),
            _ => Err("invalid array method"),
        };
        match res {
            Ok(output) => current.push_str(&output),
            Err(msg) => eprintln!("ion: {}: {}", self.method, msg),
        }
    }

    pub(crate) fn handle_as_array<E: Expander>(&self, expand_func: &E) -> Array {
        let res = match self.method {
            "split" => self.split(expand_func),
            "split_at" => self.split_at(expand_func),
            "graphemes" => self.graphemes(expand_func),
            "bytes" => self.bytes(expand_func),
            "chars" => self.chars(expand_func),
            "lines" => self.lines(expand_func),
            _ => Err("invalid array method"),
        };

        res.unwrap_or_else(|m| {
            eprintln!("ion: {}: {}", self.method, m);
            array![]
        })
    }

    #[inline]
    fn resolve_var<E: Expander>(&self, expand_func: &E) -> String {
        if let Some(variable) = expand_func.variable(self.variable, false) {
            variable
        } else if is_expression(self.variable) {
            expand_string(self.variable, expand_func, false).join(" ")
        } else {
            "".into()
        }
    }

    fn split<E: Expander>(&self, expand_func: &E) -> Result<Array, &'static str> {
        let variable = self.resolve_var(expand_func);
        let res = match (&self.pattern, self.selection.clone()) {
            (_, Select::None) => Some("".into()).into_iter().collect(),
            (&Pattern::StringPattern(pattern), Select::All) => variable
                .split(&unescape(&expand_string(pattern, expand_func, false)
                    .join(" "))?)
                .map(From::from)
                .collect(),
            (&Pattern::Whitespace, Select::All) => variable
                .split(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .map(From::from)
                .collect(),
            (&Pattern::StringPattern(pattern), Select::Index(Index::Forward(id))) => variable
                .split(&unescape(&expand_string(pattern, expand_func, false)
                    .join(" "))?)
                .nth(id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::Whitespace, Select::Index(Index::Forward(id))) => variable
                .split(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .nth(id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::StringPattern(pattern), Select::Index(Index::Backward(id))) => variable
                .rsplit(&unescape(&expand_string(pattern, expand_func, false)
                    .join(" "))?)
                .nth(id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::Whitespace, Select::Index(Index::Backward(id))) => variable
                .rsplit(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .nth(id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::StringPattern(pattern), Select::Range(range)) => {
                let expansion = unescape(&expand_string(pattern, expand_func, false).join(" "))?;
                let iter = variable.split(&expansion);
                if let Some((start, length)) = range.bounds(iter.clone().count()) {
                    iter.skip(start).take(length).map(From::from).collect()
                } else {
                    Array::new()
                }
            }
            (&Pattern::Whitespace, Select::Range(range)) => {
                let len = variable
                    .split(char::is_whitespace)
                    .filter(|x| !x.is_empty())
                    .count();
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
        Ok(res)
    }

    fn split_at<E: Expander>(&self, expand_func: &E) -> Result<Array, &'static str> {
        let variable = self.resolve_var(expand_func);
        match self.pattern {
            Pattern::StringPattern(string) => if let Ok(value) =
                expand_string(string, expand_func, false)
                    .join(" ")
                    .parse::<usize>()
            {
                if value < variable.len() {
                    let (l, r) = variable.split_at(value);
                    Ok(array![SmallString::from(l), SmallString::from(r)])
                } else {
                    Err("value is out of bounds")
                }
            } else {
                Err("requires a valid number as an argument")
            },
            Pattern::Whitespace => Err("requires an argument"),
        }
    }

    fn graphemes<E: Expander>(&self, expand_func: &E) -> Result<Array, &'static str> {
        let variable = self.resolve_var(expand_func);
        let graphemes: Vec<String> = UnicodeSegmentation::graphemes(variable.as_str(), true)
            .map(From::from)
            .collect();
        let len = graphemes.len();
        Ok(graphemes.into_iter().select(self.selection.clone(), len))
    }

    fn bytes<E: Expander>(&self, expand_func: &E) -> Result<Array, &'static str> {
        let variable = self.resolve_var(expand_func);
        let len = variable.as_bytes().len();
        Ok(variable
            .bytes()
            .map(|b| b.to_string())
            .select(self.selection.clone(), len))
    }

    fn chars<E: Expander>(&self, expand_func: &E) -> Result<Array, &'static str> {
        let variable = self.resolve_var(expand_func);
        let len = variable.chars().count();
        Ok(variable
            .chars()
            .map(|c| c.to_string())
            .select(self.selection.clone(), len))
    }

    fn lines<E: Expander>(&self, expand_func: &E) -> Result<Array, &'static str> {
        let variable = self.resolve_var(expand_func);
        Ok(variable.lines().into_iter().map(|line| line.to_string()).collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::Key;
    use super::super::super::Range;
    use types::Value;

    struct VariableExpander;

    impl Expander for VariableExpander {
        fn variable(&self, variable: &str, _: bool) -> Option<Value> {
            match variable {
                "FOO" => Some("FOOBAR".to_owned()),
                "SPACEDFOO" => Some("FOO BAR".to_owned()),
                "MULTILINE" => Some("FOO\nBAR".to_owned()),
                _ => None,
            }
        }
    }

    #[test]
    fn test_split_string_all() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$FOO",
            pattern: Pattern::StringPattern("OB"),
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_all() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FOO BAR");
    }

    #[test]
    fn test_split_string_index_forward() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$FOO",
            pattern: Pattern::StringPattern("OB"),
            selection: Select::Index(Index::Forward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "AR");
    }

    #[test]
    fn test_split_whitespace_index_forward() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::Index(Index::Forward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "BAR");
    }

    #[test]
    fn test_split_string_index_backward() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$FOO",
            pattern: Pattern::StringPattern("OB"),
            selection: Select::Index(Index::Backward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FO");
    }

    #[test]
    fn test_split_whitespace_index_backward() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::Index(Index::Backward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FOO");
    }

    #[test]
    fn test_split_string_range() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$FOO",
            pattern: Pattern::StringPattern("OB"),
            selection: Select::Range(Range::from(Index::Forward(0))),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_range() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::Range(Range::from(Index::Forward(0))),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "FOO BAR");
    }

    #[test]
    fn test_split_none() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::None,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "");
    }

    #[test]
    fn test_split_key() {
        let mut output = String::new();
        let method = ArrayMethod {
            method: "split",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::Key(Key::new("1")),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(output, "");
    }

    #[test]
    fn test_split_at_failing_whitespace() {
        let method = ArrayMethod {
            method: "split_at",
            variable: "$SPACEDFOO",
            pattern: Pattern::Whitespace,
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), array![]);
    }

    #[test]
    fn test_split_at_failing_no_number() {
        let method = ArrayMethod {
            method: "split_at",
            variable: "$SPACEDFOO",
            pattern: Pattern::StringPattern("a"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), array![]);
    }

    #[test]
    fn test_split_at_failing_out_of_bound() {
        let method = ArrayMethod {
            method: "split_at",
            variable: "$SPACEDFOO",
            pattern: Pattern::StringPattern("100"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), array![]);
    }

    #[test]
    fn test_split_at_succeeding() {
        let method = ArrayMethod {
            method: "split_at",
            variable: "$FOO",
            pattern: Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(
            method.handle_as_array(&VariableExpander),
            array!["FOO", "BAR"]
        );
    }

    #[test]
    fn test_graphemes() {
        let method = ArrayMethod {
            method: "graphemes",
            variable: "$FOO",
            pattern: Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(
            method.handle_as_array(&VariableExpander),
            array!["F", "O", "O", "B", "A", "R"]
        );
    }

    #[test]
    fn test_bytes() {
        let method = ArrayMethod {
            method: "bytes",
            variable: "$FOO",
            pattern: Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(
            method.handle_as_array(&VariableExpander),
            array!["70", "79", "79", "66", "65", "82"]
        );
    }

    #[test]
    fn test_chars() {
        let method = ArrayMethod {
            method: "chars",
            variable: "$FOO",
            pattern: Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(
            method.handle_as_array(&VariableExpander),
            array!["F", "O", "O", "B", "A", "R"]
        );
    }

    #[test]
    fn test_lines() {
        let method = ArrayMethod {
            method:    "lines",
            variable:  "$MULTILINE",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(
            method.handle_as_array(&VariableExpander),
            array!["FOO", "BAR"]
        );
    }
}
