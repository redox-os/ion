use crate::{
    parser::{shell_expand::Result, Expander},
    ranges, types,
};

/// The expression given to a for loop as the value to iterate upon.
pub enum ForValueExpression {
    Multiple(Vec<types::Str>),
    Normal(types::Str),
    Range(Box<dyn Iterator<Item = small::String> + 'static>),
}

impl ForValueExpression {
    pub fn new<E: Expander>(
        expression: &[types::Str],
        expanders: &E,
    ) -> Result<ForValueExpression> {
        let output = Vec::new();
        for exp in expression {
            output.extend(expanders.expand_string(exp)?);
        }

        Ok(if output.is_empty() {
            ForValueExpression::Multiple(output)
        } else if let (Some(range), true) = (ranges::parse_range(&output[0]), output.len() == 1) {
            ForValueExpression::Range(range)
        } else if output.len() > 1 {
            ForValueExpression::Multiple(output)
        } else {
            ForValueExpression::Normal(output[0].clone())
        })
    }
}
