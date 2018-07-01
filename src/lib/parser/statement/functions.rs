use super::split_pattern;
use lexers::assignments::{KeyBuf, KeyIterator, TypeError};

/// The arguments expression given to a function declaration goes into here, which will be
/// converted into a tuple consisting of a `KeyIterator` iterator, which will collect type
/// information, and an optional description of the function.
pub(crate) fn parse_function(arg: &str) -> (KeyIterator, Option<&str>) {
    let (args, description) = split_pattern(arg, "--");
    (KeyIterator::new(args), description)
}

/// All type information will be collected from the `KeyIterator` and stored into a vector. If a
/// type error is detected, then that error will be returned instead. This is required because
/// of lifetime restrictions on `KeyIterator`, which will not live for the remainder of the
/// declared function's lifetime.
pub(crate) fn collect_arguments(args: KeyIterator) -> Result<Vec<KeyBuf>, TypeError> {
    args.map(|a| a.map(Into::into)).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        super::super::assignments::{KeyBuf, Primitive}, *,
    };

    #[test]
    fn function_parsing() {
        let (arg_iter, description) = parse_function("a:int b:bool c[] d -- description");
        let args = collect_arguments(arg_iter);
        assert_eq!(
            args,
            Ok(vec![
                KeyBuf {
                    name: "a".into(),
                    kind: Primitive::Integer,
                },
                KeyBuf {
                    name: "b".into(),
                    kind: Primitive::Boolean,
                },
                KeyBuf {
                    name: "c".into(),
                    kind: Primitive::AnyArray,
                },
                KeyBuf {
                    name: "d".into(),
                    kind: Primitive::Any,
                },
            ])
        );
        assert_eq!(description, Some("description"))
    }
}
