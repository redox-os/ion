use super::super::types::parse::{TypeArgBuf, TypeError, TypeParser};

fn split_comments<'a>(arg: &'a str) -> (&'a str, Option<&'a str>) {
    match arg.find("--") {
        Some(pos) => {
            let args = &arg[..pos].trim();
            let comment = &arg[pos + 2..].trim();
            if comment.is_empty() { (args, None) } else { (args, Some(comment)) }
        }
        None => (arg, None),
    }
}

pub fn parse_function<'a>(arg: &'a str) -> (TypeParser<'a>, Option<&'a str>) {
    let (args, description) = split_comments(arg);
    (TypeParser::new(args), description)
}

pub fn collect_arguments<'a>(args: TypeParser<'a>) -> Result<Vec<TypeArgBuf>, TypeError<'a>> {
    let mut output: Vec<TypeArgBuf> = Vec::new();
    for arg in args {
        output.push(arg?.into());
    }
    Ok(output)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_parsing() {
        let (args, description) = split_comments("a:int b:bool -- a comment");
        assert_eq!(args, "a:int b:bool");
        assert_eq!(description, Some("a comment"));
    }
}
