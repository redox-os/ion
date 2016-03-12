use super::peg::Pipeline;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub pipelines: Vec<Pipeline>,
    pub args: Vec<String>
}
