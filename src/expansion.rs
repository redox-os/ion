use super::peg::Job;
use super::Variables;

pub fn expand_variables(mut jobs: Vec<Job>, variables: &Variables) -> Vec<Job> {
    for mut job in &mut jobs {
        job.command = expand_string(job.command.as_str(), variables);
        job.args = job.args
                      .iter()
                      .map(|original: &String| expand_string(original.as_str(), variables))
                      .collect();
    }
    jobs
}

#[inline]
fn expand_string(original: &str, variables: &Variables) -> String {
    if original.starts_with("$") {
        if let Some(value) = variables.get(&original[1..original.len()]) {
            value.clone()
        } else {
            String::new()
        }
    } else {
        original.to_string()
    }
}
