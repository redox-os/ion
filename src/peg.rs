use self::grammar::job_list;

#[derive(Debug, PartialEq)]
pub struct Job {
    pub command: String,
    pub args: Vec<String>,
}

impl Job {
    fn new(command: String, args: Vec<String>) -> Job {
        Job {
            command: command,
            args: args,
        }
    }
}

peg! grammar(r#"
use super::Job;

#[pub]
job_list  -> Vec<Job>
    = job ** job_ending

job -> Job
    = command:word whitespace args:word ** whitespace whitespace? { Job::new(command, args) }
    / command:word { Job::new(command, vec![]) }

word -> String
    = [^ \t\r\n;]+ { match_str.to_string() }

whitespace -> ()
    = [ \t]+

job_ending -> ()
    = [;\r\n]
"#);


#[cfg(test)]
mod tests {
    use super::*;
    use super::grammar::*;

    #[test]
    fn single_job_no_args() {
        let jobs = job_list("cat").unwrap();
        assert_eq!(1, jobs.len());
        assert_eq!("cat", jobs[0].command);
        assert_eq!(0, jobs[0].args.len());
    }

    #[test]
    fn single_job_with_args() {
        let jobs = job_list("ls -al dir").unwrap();
        assert_eq!(1, jobs.len());
        assert_eq!("ls", jobs[0].command);
        assert_eq!("-al", jobs[0].args[0]);
        assert_eq!("dir", jobs[0].args[1]);
    }

    #[test]
    fn multiple_jobs_with_args() {
        let jobs = job_list("ls -al;cat tmp.txt").unwrap();
        assert_eq!(2, jobs.len());
        assert_eq!("ls", jobs[0].command);
        assert_eq!("-al", jobs[0].args[0]);
        assert_eq!("cat", jobs[1].command);
        assert_eq!("tmp.txt", jobs[1].args[0]);
    }

    #[test]
    fn parse_empty_string() {
        let jobs = job_list("").unwrap();
        assert_eq!(0, jobs.len());
    }

    #[test]
    fn multiple_white_space_between_words() {
        let jobs = job_list("ls \t -al\t\tdir").unwrap();
        assert_eq!(1, jobs.len());
        assert_eq!("ls", jobs[0].command);
        assert_eq!("-al", jobs[0].args[0]);
        assert_eq!("dir", jobs[0].args[1]);
    }

    #[test]
    fn trailing_whitespace() {
        let jobs = job_list("ls -al\t ").unwrap();
        assert_eq!(1, jobs.len());
        assert_eq!("ls", jobs[0].command);
        assert_eq!("-al", jobs[0].args[0]);
    }

    // fn double_quoting()
    // fn single_quoting()
    // fn single_quoting_with_inner_double_quotes()
    // fn double_quoting_with_inner_single_quotes()
    // fn escape_character()
    // fn quoting_with_escape_character()
}
