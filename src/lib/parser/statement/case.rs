use crate::{parser::lexers::ArgumentSplitter, shell::flow_control::Case};
use err_derive::Error;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Error)]
pub enum Error {
    #[error(display = "no bind variable was supplied")]
    NoBindVariable,
    #[error(display = "no conditional statement was given")]
    NoConditional,
    #[error(display = "extra value, '{}', was given to bind", _0)]
    ExtraBind(String),
    #[error(display = "extra variable, '{}', was given to case", _0)]
    ExtraVar(String),
}

impl<'a> FromStr for Case<'a> {
    type Err = Error;

    fn from_str(data: &str) -> Result<Self, Self::Err> {
        if data == "_" {
            return Ok(Case::default());
        }
        let mut splitter = ArgumentSplitter::new(data);
        // let argument = splitter.next().ok_or(CaseError::Empty)?;
        let mut argument = None;
        let mut binding = None;
        let mut conditional = None;
        loop {
            match splitter.next() {
                Some("@") => {
                    binding = Some(splitter.next().ok_or(Error::NoBindVariable)?);
                    match splitter.next() {
                        Some("if") => {
                            // Joining by folding is more efficient than collecting into Vec and
                            // then joining
                            let mut string =
                                splitter.fold(String::with_capacity(5), |mut state, element| {
                                    state.push_str(element);
                                    state.push(' ');
                                    state
                                });
                            string.pop(); // Pop out the unneeded ' ' character
                            if string.is_empty() {
                                return Err(Error::NoConditional);
                            }
                            conditional = Some(string);
                        }
                        Some(value) => return Err(Error::ExtraBind(value.into())),
                        None => (),
                    }
                }
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
                        return Err(Error::NoConditional);
                    }
                    conditional = Some(string);
                }
                Some(inner) if argument.is_none() => {
                    argument = Some(inner);
                    continue;
                }
                Some(inner) => return Err(Error::ExtraVar(inner.into())),
                None => (),
            }
            return Ok(Case {
                value: argument.filter(|&val| val != "_").map(Into::into),
                binding: binding.map(Into::into),
                conditional,
                statements: Vec::new(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_parsing() {
        assert_eq!(
            Ok(Case {
                value:       Some("test".into()),
                binding:     Some("test".into()),
                conditional: Some("exists".into()),
                statements:  Vec::new(),
            }),
            "test @ test if exists".parse::<Case>()
        );
        assert_eq!(
            Ok(Case {
                value:       Some("test".into()),
                binding:     Some("test".into()),
                conditional: None,
                statements:  Vec::new(),
            }),
            "test @ test".parse::<Case>()
        );
        assert_eq!(
            Ok(Case {
                value:       Some("test".into()),
                binding:     None,
                conditional: None,
                statements:  Vec::new(),
            }),
            "test".parse::<Case>()
        );
    }
}
