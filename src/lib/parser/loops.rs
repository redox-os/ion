use parser::{expand_string, Expander};
use types;

#[derive(Debug, PartialEq)]
pub(crate) enum ForExpression {
    Multiple(Vec<types::Str>),
    Normal(types::Str),
    Range(usize, usize),
}

impl ForExpression {
    pub(crate) fn new<E: Expander>(expression: &[types::Str], expanders: &E) -> ForExpression {
        let output: Vec<_> = expression
            .iter()
            .flat_map(|expression| expand_string(expression, expanders, true))
            .collect();

        if output.len() == 1 {
            let output = output.into_iter().next().unwrap();
            {
                let mut bytes_iterator = output.bytes().enumerate();
                while let Some((id, byte)) = bytes_iterator.next() {
                    match byte {
                        b'0'...b'9' => continue,
                        b'.' => match output[0..id].parse::<usize>().ok() {
                            Some(first_number) => {
                                let mut dots = 1;
                                for (_, byte) in bytes_iterator {
                                    if byte == b'.' {
                                        dots += 1
                                    } else {
                                        break;
                                    }
                                }

                                match output[id + dots..].parse::<usize>().ok() {
                                    Some(second_number) => match dots {
                                        2 => {
                                            return ForExpression::Range(first_number, second_number)
                                        }
                                        3 => {
                                            return ForExpression::Range(
                                                first_number,
                                                second_number + 1,
                                            )
                                        }
                                        _ => break,
                                    },
                                    None => break,
                                }
                            }
                            None => break,
                        },
                        _ => break,
                    }
                }
            }
            ForExpression::Normal(output)
        } else {
            ForExpression::Multiple(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shell::variables::Variables;

    struct VariableExpander(pub Variables);

    impl Expander for VariableExpander {
        fn string(&self, var: &str, _: bool) -> Option<types::Str> { self.0.get::<types::Str>(var) }
    }

    #[test]
    fn for_inclusive_range() {
        let variables = Variables::default();
        let input = &["1...10".into()];
        assert_eq!(
            ForExpression::new(input, &VariableExpander(variables)),
            ForExpression::Range(1, 11)
        );
    }

    #[test]
    fn for_exclusive_range() {
        let variables = Variables::default();
        let input = &["1..10".into()];
        assert_eq!(
            ForExpression::new(input, &VariableExpander(variables)),
            ForExpression::Range(1, 10)
        );
    }

    #[test]
    fn for_normal() {
        let variables = Variables::default();
        let output = vec!["1".into(), "2".into(), "3".into(), "4".into(), "5".into()];
        assert_eq!(
            ForExpression::new(&output.clone(), &VariableExpander(variables)),
            ForExpression::Multiple(output)
        );
    }

    #[test]
    fn for_variable() {
        let mut variables = Variables::default();
        variables.set("A", "1 2 3 4 5".to_string());
        assert_eq!(
            ForExpression::new(&["$A".into()], &VariableExpander(variables)),
            ForExpression::Normal("1 2 3 4 5".into())
        );
    }
}
