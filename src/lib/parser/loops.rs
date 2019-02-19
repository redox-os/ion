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

        if output.len() == 1 {
            let output = output.into_iter().next().unwrap();
            if let Some(range) = crate::ranges::parse_range(&output) {
                return ForValueExpression::Range(range);
            }

            ForValueExpression::Normal(output)
        } else {
            ForValueExpression::Multiple(output)
        }
    }
}
