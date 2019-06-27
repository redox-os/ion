use super::{Expander, Result};
use crate::{ranges, types};

/// The expression given to a for loop as the value to iterate upon.
pub enum ForValueExpression {
    /// A set of values
    Multiple(Vec<types::Str>),
    /// A single value
    Normal(types::Str),
    /// A range of numbers
    Range(Box<dyn Iterator<Item = types::Str> + 'static>),
}

impl ForValueExpression {
    /// Parse the arguments for the for loop
    pub fn new<E: Expander>(expression: &[types::Str], expanders: &E) -> Result<Self, E::Error> {
        let mut output = Vec::new();
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
