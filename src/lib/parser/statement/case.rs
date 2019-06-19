use crate::parser::lexers::ArgumentSplitter;
use err_derive::Error;

#[derive(Debug, PartialEq, Error)]
pub enum CaseError {
    #[error(display = "no bind variable was supplied")]
    NoBindVariable,
    #[error(display = "no conditional statement was given")]
    NoConditional,
    #[error(display = "extra value, '{}', was given to bind", _0)]
    ExtraBind(String),
    #[error(display = "extra variable, '{}', was given to case", _0)]
    ExtraVar(String),
}

pub fn parse_case(data: &str) -> Result<(Option<&str>, Option<&str>, Option<String>), CaseError> {
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
                        // Joining by folding is more efficient than collecting into Vec and then
                        // joining
                        let mut string =
                            splitter.fold(String::with_capacity(5), |mut state, element| {
                                state.push_str(element);
                                state.push(' ');
                                state
                            });
                        string.pop(); // Pop out the unneeded ' ' character
                        if string.is_empty() {
                            return Err(CaseError::NoConditional);
                        }
                        conditional = Some(string);
                    }
                    Some(value) => return Err(CaseError::ExtraBind(value.into())),
                    None => (),
                }
            }
            Some("if") => {
                // Joining by folding is more efficient than collecting into Vec and then joining
                let mut string = splitter.fold(String::with_capacity(5), |mut state, element| {
                    state.push_str(element);
                    state.push(' ');
                    state
                });
                string.pop(); // Pop out the unneeded ' ' character
                if string.is_empty() {
                    return Err(CaseError::NoConditional);
                }
                conditional = Some(string);
            }
            Some(inner) if argument.is_none() => {
                argument = Some(inner);
                continue;
            }
            Some(inner) => return Err(CaseError::ExtraVar(inner.into())),
            None => (),
        }
        return Ok((argument, binding, conditional));
    }
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
        assert_eq!(Ok((Some("test"), Some("test"), None)), parse_case("test @ test"));
        assert_eq!(Ok((Some("test"), None, None)), parse_case("test"));
    }
}
