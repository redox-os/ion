use super::peg::Job;
use super::Shell;

pub trait Expand {
    fn expand_variables(&self, jobs: &mut [Job]);
    fn expand_string<'a>(&'a self, original: &'a str) -> &'a str;
}

impl Expand for Shell {
    fn expand_variables(&self, jobs: &mut [Job]) {
        for mut job in &mut jobs[..] {
            job.command = self.expand_string(&job.command).to_string();
            job.args = job.args
                .iter()
                .map(|original: &String| self.expand_string(&original).to_string())
                .collect();
        }
    }

    #[inline]
    fn expand_string<'a>(&'a self, original: &'a str) -> &'a str {
        if original.starts_with("$") {
            if let Some(value) = self.variables.get(&original[1..]) {
                &value
            } else {
                ""
            }
        } else {
            original
        }
    }
}
