use super::split_pattern;
use super::super::types::parse::{KeyBuf, KeyIterator, TypeError};

/// The arguments expression given to a function declaration goes into here, which will be
/// converted into a tuple consisting of a `KeyIterator` iterator, which will collect type
/// information, and an optional description of the function.
pub fn parse_function<'a>(arg: &'a str) -> (KeyIterator<'a>, Option<&'a str>) {
    let (args, description) = split_pattern(arg, "--");
    (KeyIterator::new(args), description)
}

/// All type information will be collected from the `KeyIterator` and stored into a vector. If a
/// type error is detected, then that error will be returned instead. This is required because
/// of lifetime restrictions on `KeyIterator`, which will not live for the remainder of the
/// declared function's lifetime.
pub fn collect_arguments<'a>(args: KeyIterator<'a>) -> Result<Vec<KeyBuf>, TypeError<'a>> {
    // NOTE: Seems to be some kind of issue with Rust's compiler accepting this:
    //     Ok(args.map(|a| a.map(Into::into)?).collect::<Vec<_>>())
    // Seems to think that `a` is a `KeyBuf` when it's actually a `Result<Key, _>`.
    let mut output = Vec::new();
    for arg in args {
        output.push(arg?.into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::types::parse::{KeyBuf, Primitive};

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
