use crate::{
    parser::{expand_string, Expander},
    types,
};

/// The expression given to a for loop as the value to iterate upon.
pub(crate) enum ForValueExpression {
    Multiple(Vec<types::Str>),
    Normal(types::Str),
    Range(Box<Iterator<Item = ::small::String> + 'static>),
}

impl ForValueExpression {
    pub(crate) fn new<E: Expander>(expression: &[types::Str], expanders: &E) -> ForValueExpression {
        let output: Vec<_> = expression
            .iter()
            .flat_map(|expression| expand_string(expression, expanders, true))
            .collect();

        if let (Some(range), true) = (crate::ranges::parse_range(&output[0]), output.len() == 1) {
            ForValueExpression::Range(range)
        } else if output.len() > 1 {
            ForValueExpression::Multiple(output)
        } else {
            ForValueExpression::Normal(output[0].clone())
        }
    }
}
