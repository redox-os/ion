use super::peg::Job;

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub jobs: Vec<Job>,
}
