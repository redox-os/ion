use super::{
    super::{
        is_expression,
        words::{Select, SelectWithSize},
    },
    strings::unescape,
    Expander,
    MethodError::*,
    Pattern,
};
use crate::types::{self, Args};
use std::char;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, PartialEq, Clone)]
pub struct ArrayMethod<'a> {
    method:    &'a str,
    variable:  &'a str,
    pattern:   Pattern<'a>,
    selection: Select,
}

impl<'a> ArrayMethod<'a> {
    pub fn new(
        method: &'a str,
        variable: &'a str,
        pattern: Pattern<'a>,
        selection: Select,
    ) -> Self {
        ArrayMethod { method, variable, pattern, selection }
    }

    fn reverse<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        let mut result = self.resolve_array(expand_func)?;
        result.reverse();
        Ok(result)
    }

    fn lines<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        Ok(self.resolve_var(expand_func)?.lines().map(types::Str::from).collect())
    }

    fn chars<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        let variable = self.resolve_var(expand_func)?;
        let len = variable.chars().count();
        Ok(variable.chars().map(|c| types::Str::from(c.to_string())).select(&self.selection, len))
    }

    fn bytes<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        let variable = self.resolve_var(expand_func)?;
        let len = variable.len();
        Ok(variable.bytes().map(|b| types::Str::from(b.to_string())).select(&self.selection, len))
    }

    fn map_keys<'b, E: Expander>(&self, expand_func: &'b E) -> super::Result<Args> {
        expand_func.map_keys(self.variable, &self.selection).ok_or(NoMapFound("map_keys"))
    }

    fn map_values<'b, E: Expander>(&self, expand_func: &'b E) -> super::Result<Args> {
        expand_func.map_values(self.variable, &self.selection).ok_or(NoMapFound("map_values"))
    }

    fn graphemes<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        let variable = self.resolve_var(expand_func)?;
        let graphemes: Vec<types::Str> =
            UnicodeSegmentation::graphemes(variable.as_str(), true).map(From::from).collect();
        let len = graphemes.len();
        Ok(graphemes.into_iter().select(&self.selection, len))
    }

    fn split_at<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        let variable = self.resolve_var(expand_func)?;
        match self.pattern {
            Pattern::StringPattern(string) => {
                if let Ok(value) = expand_func.expand_string(string)?.join(" ").parse::<usize>() {
                    if value < variable.len() {
                        let (l, r) = variable.split_at(value);
                        Ok(args![types::Str::from(l), types::Str::from(r)])
                    } else {
                        Err(OutOfBound.into())
                    }
                } else {
                    Err(WrongArgument("split_at", "requires a valid number as an argument").into())
                }
            }
            Pattern::Whitespace => Err(WrongArgument("split_at", "requires an argument").into()),
        }
    }

    fn split<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        let variable = self.resolve_var(expand_func)?;
        let data: Args = match self.pattern {
            Pattern::Whitespace => variable
                .split(char::is_whitespace)
                .filter(|x| !x.is_empty())
                .map(From::from)
                .collect(),
            Pattern::StringPattern(pattern) => variable
                .split(unescape(&expand_func.expand_string(pattern)?.join(" ")).as_str())
                .map(From::from)
                .collect(),
        };
        let len = data.len();
        Ok(data.into_iter().select(&self.selection, len))
    }

    #[inline]
    fn resolve_array<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        if let Some(array) = expand_func.array(self.variable, &Select::All) {
            Ok(array)
        } else if is_expression(self.variable) {
            expand_func.expand_string(self.variable)
        } else {
            Ok(Args::new())
        }
    }

    #[inline]
    fn resolve_var<E: Expander>(&self, expand_func: &E) -> super::super::Result<types::Str> {
        if let Some(variable) = expand_func.string(self.variable) {
            Ok(variable)
        } else if is_expression(self.variable) {
            Ok(types::Str::from_string(expand_func.expand_string(self.variable)?.join(" ")))
        } else {
            Ok("".into())
        }
    }

    pub fn handle_as_array<E: Expander>(&self, expand_func: &E) -> super::super::Result<Args> {
        match self.method {
            "bytes" => self.bytes(expand_func),
            "chars" => self.chars(expand_func),
            "graphemes" => self.graphemes(expand_func),
            "keys" => self.map_keys(expand_func).map_err(Into::into),
            "lines" => self.lines(expand_func),
            "reverse" => self.reverse(expand_func),
            "split_at" => self.split_at(expand_func).map_err(Into::into),
            "split" => self.split(expand_func),
            "values" => self.map_values(expand_func).map_err(Into::into),
            _ => Err(InvalidArrayMethod(self.method.to_string()).into()),
        }
    }

    pub fn handle<E: Expander>(
        &self,
        current: &mut types::Str,
        expand_func: &E,
    ) -> super::super::Result<()> {
        match self.method {
            "split" => Ok(current.push_str(&self.split(expand_func)?.join(" "))),
            _ => Err(InvalidArrayMethod(self.method.to_string()).into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        ranges::{Index, Range},
        shell::IonError,
        types,
    };

    struct VariableExpander;

    impl Expander for VariableExpander {
        type Error = IonError;

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
        let method = ArrayMethod::new("split", "$FOO", Pattern::StringPattern("OB"), Select::All);
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_all() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Select::All);
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_split_string_index_forward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new(
            "split",
            "$FOO",
            Pattern::StringPattern("OB"),
            Select::Index(Index::Forward(1)),
        );
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "AR");
    }

    #[test]
    fn test_split_whitespace_index_forward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new(
            "split",
            "$SPACEDFOO",
            Pattern::Whitespace,
            Select::Index(Index::Forward(1)),
        );
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "BAR");
    }

    #[test]
    fn test_split_string_index_backward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new(
            "split",
            "$FOO",
            Pattern::StringPattern("OB"),
            Select::Index(Index::Backward(1)),
        );
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "FO");
    }

    #[test]
    fn test_split_whitespace_index_backward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new(
            "split",
            "$SPACEDFOO",
            Pattern::Whitespace,
            Select::Index(Index::Backward(1)),
        );
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "FOO");
    }

    #[test]
    fn test_split_string_range() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new(
            "split",
            "$FOO",
            Pattern::StringPattern("OB"),
            Select::Range(Range::from(Index::Forward(0))),
        );
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_range() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new(
            "split",
            "$SPACEDFOO",
            Pattern::Whitespace,
            Select::Range(Range::from(Index::Forward(0))),
        );
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_split_none() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Select::None);
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "");
    }

    #[test]
    fn test_split_key() {
        let mut output = types::Str::new();
        let method =
            ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Select::Key("1".into()));
        method.handle(&mut output, &VariableExpander).unwrap();
        assert_eq!(&*output, "");
    }

    #[test]
    fn test_split_at_failing_whitespace() {
        let method = ArrayMethod::new("split_at", "$SPACEDFOO", Pattern::Whitespace, Select::All);
        assert!(method.handle_as_array(&VariableExpander).is_err());
    }

    #[test]
    fn test_split_at_failing_no_number() {
        let method =
            ArrayMethod::new("split_at", "$SPACEDFOO", Pattern::StringPattern("a"), Select::All);
        assert!(method.handle_as_array(&VariableExpander).is_err());
    }

    #[test]
    fn test_split_at_failing_out_of_bound() {
        let method =
            ArrayMethod::new("split_at", "$SPACEDFOO", Pattern::StringPattern("100"), Select::All);
        assert!(method.handle_as_array(&VariableExpander).is_err());
    }

    #[test]
    fn test_split_at_succeeding() {
        let method = ArrayMethod::new("split_at", "$FOO", Pattern::StringPattern("3"), Select::All);
        assert_eq!(method.handle_as_array(&VariableExpander).unwrap(), args!["FOO", "BAR"]);
    }

    #[test]
    fn test_graphemes() {
        let method =
            ArrayMethod::new("graphemes", "$FOO", Pattern::StringPattern("3"), Select::All);
        assert_eq!(
            method.handle_as_array(&VariableExpander).unwrap(),
            args!["F", "O", "O", "B", "A", "R"]
        );
    }

    #[test]
    fn test_bytes() {
        let method = ArrayMethod::new("bytes", "$FOO", Pattern::StringPattern("3"), Select::All);
        assert_eq!(
            method.handle_as_array(&VariableExpander).unwrap(),
            args!["70", "79", "79", "66", "65", "82"]
        );
    }

    #[test]
    fn test_chars() {
        let method = ArrayMethod::new("chars", "$FOO", Pattern::StringPattern("3"), Select::All);
        assert_eq!(
            method.handle_as_array(&VariableExpander).unwrap(),
            args!["F", "O", "O", "B", "A", "R"]
        );
    }

    #[test]
    fn test_lines() {
        let method =
            ArrayMethod::new("lines", "$MULTILINE", Pattern::StringPattern("3"), Select::All);
        assert_eq!(method.handle_as_array(&VariableExpander).unwrap(), args!["FOO", "BAR"]);
    }

    #[test]
    fn test_reverse() {
        let method =
            ArrayMethod::new("reverse", "@ARRAY", Pattern::StringPattern("3"), Select::All);
        assert_eq!(method.handle_as_array(&VariableExpander).unwrap(), args!["c", "b", "a"]);
    }
}
