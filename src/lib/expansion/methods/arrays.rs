use super::{
    super::{is_expression, words::Select, Error, Expander, ExpanderInternal, Index},
    strings::unescape,
    MethodError, Pattern,
};
use crate::{
    expansion::is_array_expression,
    types::{self, Args},
};
use std::char;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, PartialEq, Clone)]
pub struct ArrayMethod<'a> {
    method:    &'a str,
    variable:  &'a str,
    pattern:   Pattern<'a>,
    selection: Option<&'a str>,
}

impl<'a> ArrayMethod<'a> {
    pub const fn new(
        method: &'a str,
        variable: &'a str,
        pattern: Pattern<'a>,
        selection: Option<&'a str>,
    ) -> Self {
        Self { method, variable, pattern, selection }
    }

    fn reverse<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let mut result = self.resolve_array(expand_func)?;
        result.reverse();
        Ok(result)
    }

    fn lines<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        Ok(self.resolve_var(expand_func)?.lines().map(types::Str::from).collect())
    }

    fn chars<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let variable = self.resolve_var(expand_func)?;
        expand_func
            .slice_array(variable.chars().map(|c| types::Str::from(c.to_string())), &self.selection)
    }

    fn bytes<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let variable = self.resolve_var(expand_func)?;
        expand_func
            .slice_array(variable.bytes().map(|b| types::Str::from(b.to_string())), &self.selection)
    }

    fn map_keys<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        expand_func.slice_array(expand_func.map_keys(self.variable)?.into_iter(), &self.selection)
    }

    fn map_values<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        expand_func.slice_array(expand_func.map_values(self.variable)?.into_iter(), &self.selection)
    }

    fn graphemes<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let variable = self.resolve_var(expand_func)?;
        let graphemes = UnicodeSegmentation::graphemes(variable.as_str(), true);
        expand_func.slice_array(graphemes, &self.selection)
    }

    fn split_at<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let variable = self.resolve_var(expand_func)?;
        match self.pattern {
            Pattern::StringPattern(string) => {
                if let Ok(value) = expand_func.expand_string(string)?.join(" ").parse::<usize>() {
                    if value < variable.len() {
                        let (l, r) = variable.split_at(value);
                        Ok(args![types::Str::from(l), types::Str::from(r)])
                    } else {
                        Err(Error::InvalidIndex(
                            Select::Index(Index::Forward(value)),
                            "array",
                            variable.to_string(),
                        ))
                    }
                } else {
                    Err(MethodError::WrongArgument(
                        "split_at",
                        "requires a valid number as an argument",
                    )
                    .into())
                }
            }
            Pattern::Whitespace => {
                Err(MethodError::WrongArgument("split_at", "requires an argument").into())
            }
        }
    }

    fn split<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let variable = self.resolve_var(expand_func)?;
        match self.pattern {
            Pattern::Whitespace => {
                let data = variable.split(char::is_whitespace).filter(|x| !x.is_empty());
                expand_func.slice_array(data, &self.selection)
            }
            Pattern::StringPattern(pattern) => {
                let escape = unescape(&expand_func.expand_string(pattern)?.join(" "));
                let data = variable.split(escape.as_str());
                expand_func.slice_array(data, &self.selection)
            }
        }
    }

    fn subst<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        let variable = self.resolve_var(expand_func)?;

        if !is_array_expression(&self.variable) {
            return Err(MethodError::WrongArgument(
                stringify!(subst),
                "1. argument must be an array",
            )
            .into());
        }
        let default_over_empty = match self.pattern {
            Pattern::StringPattern(pattern) => {
                if !is_array_expression(pattern) {
                    return Err(MethodError::WrongArgument(
                        stringify!(subst),
                        "2. argument must be an array",
                    )
                    .into());
                }
                Self::resolve_arg_array(expand_func, pattern)
            }
            Pattern::Whitespace => {
                Err(MethodError::WrongArgument(stringify!(subst), "requires a 2. argument").into())
            }
        }?;

        let elements = variable.split(char::is_whitespace);

        let array = expand_func.slice_array(elements, &self.selection)?;

        if array.is_empty()
            || array.get(0).expect("Unexpected: array was checked if it was empty").is_empty()
        {
            Ok(default_over_empty)
        } else {
            Ok(array)
        }
    }

    #[inline]
    fn resolve_array<E: Expander>(&self, expand_func: &mut E) -> Result<Args, Error<E::Error>> {
        match expand_func.array(self.variable, &Select::All) {
            Ok(array) => Ok(array),
            Err(Error::VarNotFound) if is_expression(self.variable) => {
                expand_func.expand_string(self.variable)
            }
            Err(why) => Err(why),
        }
    }

    fn resolve_arg_array<E: Expander>(
        expand_func: &mut E,
        to_resolve: &str,
    ) -> Result<Args, Error<E::Error>> {
        match expand_func.array(to_resolve, &Select::All) {
            Ok(array) => Ok(array),
            Err(Error::VarNotFound) if is_expression(to_resolve) => {
                expand_func.expand_string(to_resolve)
            }
            Err(why) => Err(why),
        }
    }

    #[inline]
    fn resolve_var<E: Expander>(&self, expand_func: &mut E) -> Result<types::Str, Error<E::Error>> {
        match expand_func.string(self.variable) {
            Ok(variable) => Ok(variable),
            Err(Error::VarNotFound) if is_expression(self.variable) => {
                Ok(types::Str::from_string(expand_func.expand_string(self.variable)?.join(" ")))
            }
            Err(why) => Err(why),
        }
    }

    pub fn handle_as_array<E: Expander>(
        &self,
        expand_func: &mut E,
    ) -> Result<Args, Error<E::Error>> {
        match self.method {
            "bytes" => self.bytes(expand_func),
            "chars" => self.chars(expand_func),
            "graphemes" => self.graphemes(expand_func),
            "keys" => self.map_keys(expand_func).map_err(Error::from),
            "lines" => self.lines(expand_func),
            "reverse" => self.reverse(expand_func),
            "split_at" => self.split_at(expand_func),
            "split" => self.split(expand_func),
            "values" => self.map_values(expand_func).map_err(Error::from),
            "subst" => self.subst(expand_func),
            _ => Err(MethodError::InvalidArrayMethod(self.method.to_string()).into()),
        }
    }

    pub fn handle<E: Expander>(
        &self,
        current: &mut types::Str,
        expand_func: &mut E,
    ) -> Result<(), Error<E::Error>> {
        match self.method {
            "split" => {
                current.push_str(&self.split(expand_func)?.join(" "));
                Ok(())
            }
            _ => Err(MethodError::InvalidArrayMethod(self.method.to_string()).into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{expansion::test::DummyExpander, types};

    #[test]
    fn test_split_string_all() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$FOO", Pattern::StringPattern("OB"), None);
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_all() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, None);
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_split_string_index_forward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$FOO", Pattern::StringPattern("OB"), Some("1"));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "AR");
    }

    #[test]
    fn test_split_whitespace_index_forward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Some("1"));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "BAR");
    }

    #[test]
    fn test_split_string_index_backward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$FOO", Pattern::StringPattern("OB"), Some("-2"));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FO");
    }

    #[test]
    fn test_split_whitespace_index_backward() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Some("-2"));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOO");
    }

    #[test]
    fn test_split_string_range() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$FOO", Pattern::StringPattern("OB"), Some("0.."));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FO AR");
    }

    #[test]
    fn test_split_whitespace_range() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Some("0.."));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "FOO BAR");
    }

    #[test]
    fn test_split_key() {
        let mut output = types::Str::new();
        let method = ArrayMethod::new("split", "$SPACEDFOO", Pattern::Whitespace, Some("\"1\""));
        method.handle(&mut output, &mut DummyExpander).unwrap();
        assert_eq!(&*output, "BAR");
    }

    #[test]
    fn test_split_at_failing_whitespace() {
        let method = ArrayMethod::new("split_at", "$SPACEDFOO", Pattern::Whitespace, None);
        assert!(method.handle_as_array(&mut DummyExpander).is_err());
    }

    #[test]
    fn test_split_at_failing_no_number() {
        let method = ArrayMethod::new("split_at", "$SPACEDFOO", Pattern::StringPattern("a"), None);
        assert!(method.handle_as_array(&mut DummyExpander).is_err());
    }

    #[test]
    fn test_split_at_failing_out_of_bound() {
        let method =
            ArrayMethod::new("split_at", "$SPACEDFOO", Pattern::StringPattern("100"), None);
        assert!(method.handle_as_array(&mut DummyExpander).is_err());
    }

    #[test]
    fn test_split_at_succeeding() {
        let method = ArrayMethod::new("split_at", "$FOO", Pattern::StringPattern("3"), None);
        assert_eq!(method.handle_as_array(&mut DummyExpander).unwrap(), args!["FOO", "BAR"]);
    }

    #[test]
    fn test_graphemes() {
        let method = ArrayMethod::new("graphemes", "$FOO", Pattern::StringPattern("3"), None);
        assert_eq!(
            method.handle_as_array(&mut DummyExpander).unwrap(),
            args!["F", "O", "O", "B", "A", "R"]
        );
    }

    #[test]
    fn test_bytes() {
        let method = ArrayMethod::new("bytes", "$FOO", Pattern::StringPattern("3"), None);
        assert_eq!(
            method.handle_as_array(&mut DummyExpander).unwrap(),
            args!["70", "79", "79", "66", "65", "82"]
        );
    }

    #[test]
    fn test_chars() {
        let method = ArrayMethod::new("chars", "$FOO", Pattern::StringPattern("3"), None);
        assert_eq!(
            method.handle_as_array(&mut DummyExpander).unwrap(),
            args!["F", "O", "O", "B", "A", "R"]
        );
    }

    #[test]
    fn test_lines() {
        let method = ArrayMethod::new("lines", "$MULTILINE", Pattern::StringPattern("3"), None);
        assert_eq!(method.handle_as_array(&mut DummyExpander).unwrap(), args!["FOO", "BAR"]);
    }

    #[test]
    fn test_reverse() {
        let method = ArrayMethod::new("reverse", "@ARRAY", Pattern::StringPattern("3"), None);
        assert_eq!(method.handle_as_array(&mut DummyExpander).unwrap(), args!["c", "b", "a"]);
    }

    #[test]
    fn test_subst_variable_over_default() {
        let method = ArrayMethod::new("subst", "@ARRAY", Pattern::StringPattern("[2, 3]"), None);
        assert_eq!(method.handle_as_array(&mut DummyExpander).unwrap(), args!["a", "b", "c"]);
    }
    #[test]
    fn test_subst_default_over_empty_array() {
        let method = ArrayMethod::new("subst", "[]", Pattern::StringPattern("[2 3]"), None);
        assert_eq!(method.handle_as_array(&mut DummyExpander).unwrap(), args!["2", "3"]);
    }

    #[test]
    fn test_subst_fail_no_array_variable() {
        let method =
            ArrayMethod::new("subst", "not an array", Pattern::StringPattern("[2 3]"), None);
        assert!(method.handle_as_array(&mut DummyExpander).is_err());
    }
    #[test]
    fn test_subst_fail_default_is_no_array() {
        let method = ArrayMethod::new("subst", "[]", Pattern::StringPattern("not an array"), None);
        assert!(method.handle_as_array(&mut DummyExpander).is_err());
    }
    #[test]
    fn test_subst_fail_default_missing() {
        let method = ArrayMethod::new("subst", "[]", Pattern::Whitespace, None);
        assert!(method.handle_as_array(&mut DummyExpander).is_err());
    }
}
