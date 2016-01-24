use super::peg::Job;
use super::Variables;

pub fn expand_variables(mut jobs: Vec<Job>, variables: &Variables) -> Vec<Job> {
    for mut job in &mut jobs {
        job.command = expand_string(&job.command, variables).to_string();
        job.args = job.args
                      .iter()
                      .map(|original: &String| expand_string(&original, variables).to_string())
                      .collect();
    }
    jobs
}

#[inline]
fn expand_string<'a>(original: &'a str, variables: &'a Variables) -> &'a str {
    if original.starts_with("$") {
        if let Some(value) = variables.get(&original[1..]) {
            &value
        } else {
            ""
        }
    } else {
        original
    }
}
