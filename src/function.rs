use super::flow_control::Expression;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub expressions: Vec<Expression>,
}
