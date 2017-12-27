use super::super::ArgumentSplitter;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq)]
pub(crate) enum CaseError<'a> {
    NoBindVariable,
    NoConditional,
    ExtraBind(&'a str),
    ExtraVar(&'a str),
}

impl<'a> Display for CaseError<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            CaseError::NoBindVariable => write!(f, "no bind variable was supplied"),
            CaseError::NoConditional => write!(f, "no conditional statement was given"),
            CaseError::ExtraBind(value) => write!(f, "extra value, '{}', was given to bind", value),
            CaseError::ExtraVar(value) => {
                write!(f, "extra variable, '{}', was given to case", value)
            }
        }
    }
}

pub(crate) fn parse_case<'a>(
    data: &'a str,
) -> Result<(Option<&'a str>, Option<&'a str>, Option<String>), CaseError<'a>> {
    let mut splitter = ArgumentSplitter::new(data);
    // let argument = splitter.next().ok_or(CaseError::Empty)?;
    let mut argument = None;
    let mut binding = None;
    let mut conditional = None;
    loop {
        match splitter.next() {
            Some("@") => {
                binding = Some(splitter.next().ok_or(CaseError::NoBindVariable)?);
                match splitter.next() {
                    Some("if") => {
                        let string = splitter.collect::<Vec<_>>().join(" ");
                        if string.is_empty() {
                            return Err(CaseError::NoConditional);
                        }
                        conditional = Some(string);
                    }
                    Some(value) => return Err(CaseError::ExtraBind(value)),
                    None => (),
                }
            }
            Some("if") => {
                let string = splitter.collect::<Vec<_>>().join(" ");
                if string.is_empty() {
                    return Err(CaseError::NoConditional);
                }
                conditional = Some(string);
            }
            Some(inner) => if argument.is_none() {
                argument = Some(inner);
                continue;
            } else {
                return Err(CaseError::ExtraVar(inner));
            },
            None => (),
        }
        break;
    }

    Ok((argument, binding, conditional))
}

#[cfg(test)]
mod tests {
    use super::parse_case;
    #[test]
    fn case_parsing() {
        assert_eq!(
            Ok((Some("test"), Some("test"), Some("exists".into()))),
            parse_case("test @ test if exists")
        );
        assert_eq!(
            Ok((Some("test"), Some("test"), None)),
            parse_case("test @ test")
        );
        assert_eq!(Ok((Some("test"), None, None)), parse_case("test"));
    }
}
