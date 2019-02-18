use super::split_pattern;
use crate::lexers::assignments::{KeyBuf, KeyIterator, TypeError};
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq)]
pub(crate) enum FunctionParseError {
    RepeatedArgument(String),
    TypeError(TypeError),
}

impl<'a> Display for FunctionParseError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            FunctionParseError::RepeatedArgument(ref arg) => {
                write!(f, "repeated argument name: '{}'", arg)
            }
            FunctionParseError::TypeError(ref t) => write!(f, "{}", t),
        }
    }
}

/// The arguments expression given to a function declaration goes into here, which will be
/// converted into a tuple consisting of a `KeyIterator` iterator, which will collect type
/// information, and an optional description of the function.
pub(crate) fn parse_function(arg: &str) -> (KeyIterator, Option<&str>) {
    let (args, description) = split_pattern(arg, "--");
    (KeyIterator::new(args), description)
}

/// All type information will be collected from the `KeyIterator` and stored into a vector. If a
/// type or argument error is detected, then that error will be returned instead. This is required
/// because of lifetime restrictions on `KeyIterator`, which will not live for the remainder of the
/// declared function's lifetime.
pub(crate) fn collect_arguments(args: KeyIterator) -> Result<Vec<KeyBuf>, FunctionParseError> {
    let mut keybuf: Vec<KeyBuf> = Vec::new();
    for arg in args {
        match arg {
            Ok(key) => {
                let key: KeyBuf = key.into();
                if keybuf.iter().any(|k| k.name == key.name) {
                    return Err(FunctionParseError::RepeatedArgument(key.name));
                } else {
                    keybuf.push(key);
                }
            }
            Err(e) => return Err(FunctionParseError::TypeError(e)),
        }
    }
    Ok(keybuf)
}

#[cfg(test)]
mod tests {
    use crate::{
        lexers::assignments::{KeyBuf, Primitive},
        parser::statement::functions::{collect_arguments, parse_function, FunctionParseError},
    };

    #[test]
    fn function_parsing() {
        let (arg_iter, description) = parse_function("a:int b:bool c[] d -- description");
        let args = collect_arguments(arg_iter);
        assert_eq!(
            args,
            Ok(vec![
                KeyBuf { name: "a".into(), kind: Primitive::Integer },
                KeyBuf { name: "b".into(), kind: Primitive::Boolean },
                KeyBuf { name: "c".into(), kind: Primitive::AnyArray },
                KeyBuf { name: "d".into(), kind: Primitive::Any },
            ])
        );
        assert_eq!(description, Some("description"))
    }

    #[test]
    fn function_repeated_arg() {
        let (arg_iter, description) = parse_function("a:bool b a[] -- failed def");
        let args = collect_arguments(arg_iter);
        assert_eq!(args, Err(FunctionParseError::RepeatedArgument("a".into())));
        assert_eq!(description, Some("failed def"));
    }
}
