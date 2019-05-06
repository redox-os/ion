use super::{
    super::{
        super::{expand_string, is_expression, Expander},
        Select, SelectWithSize,
    },
    strings::unescape,
    Pattern,
};
use crate::{
    ranges::Index,
    types::{self, Args},
};
use small;
use std::char;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ArrayMethod<'a> {
    pub(crate) method:    &'a str,
    pub(crate) variable:  &'a str,
    pub(crate) pattern:   Pattern<'a>,
    pub(crate) selection: Select,
}

impl<'a> ArrayMethod<'a> {
    fn reverse<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        let mut result = self.resolve_array(expand_func);
        result.reverse();
        Ok(result)
    }

    fn lines<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        Ok(self.resolve_var(expand_func).lines().map(types::Str::from).collect())
    }

    fn chars<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        let variable = self.resolve_var(expand_func);
        let len = variable.chars().count();
        Ok(variable.chars().map(|c| types::Str::from(c.to_string())).select(&self.selection, len))
    }

    fn bytes<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        let variable = self.resolve_var(expand_func);
        let len = variable.len();
        Ok(variable.bytes().map(|b| types::Str::from(b.to_string())).select(&self.selection, len))
    }

    fn map_keys<'b, E: Expander>(&self, expand_func: &'b E) -> Result<Args, &'static str> {
        expand_func.map_keys(self.variable, &self.selection).ok_or("no map found")
    }

    fn map_values<'b, E: Expander>(&self, expand_func: &'b E) -> Result<Args, &'static str> {
        expand_func.map_values(self.variable, &self.selection).ok_or("no map found")
    }

    fn graphemes<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        let variable = self.resolve_var(expand_func);
        let graphemes: Vec<types::Str> =
            UnicodeSegmentation::graphemes(variable.as_str(), true).map(From::from).collect();
        let len = graphemes.len();
        Ok(graphemes.into_iter().select(&self.selection, len))
    }

    fn split_at<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        let variable = self.resolve_var(expand_func);
        match self.pattern {
            Pattern::StringPattern(string) => {
                if let Ok(value) = expand_string(string, expand_func).join(" ").parse::<usize>() {
                    if value < variable.len() {
                        let (l, r) = variable.split_at(value);
                        Ok(args![types::Str::from(l), types::Str::from(r)])
                    } else {
                        Err("value is out of bounds")
                    }
                } else {
                    Err("requires a valid number as an argument")
                }
            }
            Pattern::Whitespace => Err("requires an argument"),
        }
    }

    fn split<E: Expander>(&self, expand_func: &E) -> Result<Args, &'static str> {
        let variable = self.resolve_var(expand_func);
        let res = match (&self.pattern, &self.selection) {
            (_, Select::None) => Some("".into()).into_iter().collect(),
            (&Pattern::StringPattern(pattern), Select::All) => variable
                .split(unescape(&expand_string(pattern, expand_func).join(" "))?.as_str())
                .map(From::from)
                .collect(),
            (&Pattern::Whitespace, Select::All) => variable
                .split(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .map(From::from)
                .collect(),
            (&Pattern::StringPattern(pattern), Select::Index(Index::Forward(id))) => variable
                .split(&unescape(&expand_string(pattern, expand_func).join(" "))?.as_str())
                .nth(*id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::Whitespace, Select::Index(Index::Forward(id))) => variable
                .split(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .nth(*id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::StringPattern(pattern), Select::Index(Index::Backward(id))) => variable
                .rsplit(&unescape(&expand_string(pattern, expand_func).join(" "))?.as_str())
                .nth(*id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::Whitespace, Select::Index(Index::Backward(id))) => variable
                .rsplit(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .nth(*id)
                .map(From::from)
                .into_iter()
                .collect(),
            (&Pattern::StringPattern(pattern), Select::Range(range)) => {
                let expansion = unescape(&expand_string(pattern, expand_func).join(" "))?;
                let iter = variable.split(expansion.as_str());
                if let Some((start, length)) = range.bounds(iter.clone().count()) {
                    iter.skip(start).take(length).map(From::from).collect()
                } else {
                    Args::new()
                }
            }
            (&Pattern::Whitespace, Select::Range(range)) => {
                let len = variable.split(char::is_whitespace).filter(|x| !x.is_empty()).count();
                if let Some((start, length)) = range.bounds(len) {
                    variable
                        .split(char::is_whitespace)
                        .filter(|x| !x.is_empty())
                        .skip(start)
                        .take(length)
                        .map(From::from)
                        .collect()
                } else {
                    Args::new()
                }
            }
            (_, Select::Key(_)) => Some("".into()).into_iter().collect(),
        };
        Ok(res)
    }

    #[inline]
    fn resolve_array<E: Expander>(&self, expand_func: &E) -> Args {
        if let Some(array) = expand_func.array(self.variable, &Select::All) {
            array
        } else if is_expression(self.variable) {
            expand_string(self.variable, expand_func)
        } else {
            Args::new()
        }
    }

    #[inline]
    fn resolve_var<E: Expander>(&self, expand_func: &E) -> types::Str {
        if let Some(variable) = expand_func.string(self.variable) {
            variable
        } else if is_expression(self.variable) {
            types::Str::from_string(expand_string(self.variable, expand_func).join(" "))
        } else {
            "".into()
        }
    }

    pub(crate) fn handle_as_array<E: Expander>(&self, expand_func: &E) -> Args {
        let res = match self.method {
            "bytes" => self.bytes(expand_func),
            "chars" => self.chars(expand_func),
            "graphemes" => self.graphemes(expand_func),
            "keys" => self.map_keys(expand_func),
            "lines" => self.lines(expand_func),
            "reverse" => self.reverse(expand_func),
            "split_at" => self.split_at(expand_func),
            "split" => self.split(expand_func),
            "values" => self.map_values(expand_func),
            _ => Err("invalid array method"),
        };

        res.unwrap_or_else(|m| {
            eprintln!("ion: {}: {}", self.method, m);
            Args::new()
        })
    }

    pub(crate) fn handle<E: Expander>(&self, current: &mut small::String, expand_func: &E) {
        let res = match self.method {
            "split" => self.split(expand_func).map(|r| r.join(" ")),
            _ => Err("invalid array method"),
        };
        match res {
            Ok(output) => current.push_str(&output),
            Err(msg) => eprintln!("ion: {}: {}", self.method, msg),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{ranges::Range, types};

    struct VariableExpander;

    impl Expander for VariableExpander {
        fn array(&self, variable: &str, _: &Select) -> Option<types::Args> {
            match variable {
                "ARRAY" => Some(args!["a", "b", "c"].to_owned()),
                _ => None,
            }
        }

        fn string(&self, variable: &str) -> Option<types::Str> {
            match variable {
                "FOO" => Some("FOOBAR".into()),
                "SPACEDFOO" => Some("FOO BAR".into()),
                "MULTILINE" => Some("FOO\nBAR".into()),
                _ => None,
            }
        }
    }

    #[test]
    fn test_split_string_all() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("OB"),
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_all() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::All,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_split_string_index_forward() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("OB"),
            selection: Select::Index(Index::Forward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "AR");
    }

    #[test]
    fn test_split_whitespace_index_forward() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::Index(Index::Forward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "BAR");
    }

    #[test]
    fn test_split_string_index_backward() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("OB"),
            selection: Select::Index(Index::Backward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "FO");
    }

    #[test]
    fn test_split_whitespace_index_backward() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::Index(Index::Backward(1)),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "FOO");
    }

    #[test]
    fn test_split_string_range() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("OB"),
            selection: Select::Range(Range::from(Index::Forward(0))),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_range() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::Range(Range::from(Index::Forward(0))),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_split_none() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::None,
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "");
    }

    #[test]
    fn test_split_key() {
        let mut output = types::Str::new();
        let method = ArrayMethod {
            method:    "split",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::Key("1".into()),
        };
        method.handle(&mut output, &VariableExpander);
        assert_eq!(&*output, "");
    }

    #[test]
    fn test_split_at_failing_whitespace() {
        let method = ArrayMethod {
            method:    "split_at",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::Whitespace,
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args![]);
    }

    #[test]
    fn test_split_at_failing_no_number() {
        let method = ArrayMethod {
            method:    "split_at",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::StringPattern("a"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args![]);
    }

    #[test]
    fn test_split_at_failing_out_of_bound() {
        let method = ArrayMethod {
            method:    "split_at",
            variable:  "$SPACEDFOO",
            pattern:   Pattern::StringPattern("100"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args![]);
    }

    #[test]
    fn test_split_at_succeeding() {
        let method = ArrayMethod {
            method:    "split_at",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args!["FOO", "BAR"]);
    }

    #[test]
    fn test_graphemes() {
        let method = ArrayMethod {
            method:    "graphemes",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args!["F", "O", "O", "B", "A", "R"]);
    }

    #[test]
    fn test_bytes() {
        let method = ArrayMethod {
            method:    "bytes",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(
            method.handle_as_array(&VariableExpander),
            args!["70", "79", "79", "66", "65", "82"]
        );
    }

    #[test]
    fn test_chars() {
        let method = ArrayMethod {
            method:    "chars",
            variable:  "$FOO",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args!["F", "O", "O", "B", "A", "R"]);
    }

    #[test]
    fn test_lines() {
        let method = ArrayMethod {
            method:    "lines",
            variable:  "$MULTILINE",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args!["FOO", "BAR"]);
    }

    #[test]
    fn test_reverse() {
        let method = ArrayMethod {
            method:    "reverse",
            variable:  "@ARRAY",
            pattern:   Pattern::StringPattern("3"),
            selection: Select::All,
        };
        assert_eq!(method.handle_as_array(&VariableExpander), args!["c", "b", "a"]);
    }
}
