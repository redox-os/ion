use crate::{parser::Expander, ranges, types};

/// The expression given to a for loop as the value to iterate upon.
pub enum ForValueExpression {
    Multiple(Vec<types::Str>),
    Normal(types::Str),
    Range(Box<dyn Iterator<Item = small::String> + 'static>),
}

impl ForValueExpression {
    pub fn new<E: Expander>(expression: &[types::Str], expanders: &E) -> ForValueExpression {
        let output: Vec<_> =
            expression.iter().flat_map(|expression| expanders.expand_string(expression)).collect();

        if output.is_empty() {
            ForValueExpression::Multiple(output)
        } else if let (Some(range), true) = (ranges::parse_range(&output[0]), output.len() == 1) {
            ForValueExpression::Range(range)
        } else if output.len() > 1 {
            ForValueExpression::Multiple(output)
        } else {
            ForValueExpression::Normal(output[0].clone())
        }
    }
}
