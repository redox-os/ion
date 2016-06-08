use std::process::Command;

use self::grammar::pipelines;
use self::grammar::if_;

use glob::glob;

#[derive(Debug, PartialEq, Clone)]
pub struct Redirection {
    pub file: String,
    pub append: bool
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pipeline {
    pub jobs: Vec<Job>,
    pub stdout: Option<Redirection>,
    pub stdin: Option<Redirection>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ShellUpdate {
    FlowControl(IfStatement),
    Pipelines(Vec<Pipeline>)
}

#[derive(Debug, PartialEq, Clone)]
pub struct IfStatement {
    pub left: String,
    pub comparitor: Comparitor,
    pub right: String
    //pub if_block: Pipeline,
    //pub else_block: Pipeline
}

#[derive(Debug, PartialEq, Clone)]
pub enum Comparitor {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual
}

impl Pipeline {

    pub fn new(jobs: Vec<Job>, stdin: Option<Redirection>, stdout: Option<Redirection>) -> Self {
        Pipeline {
            jobs: jobs,
            stdin: stdin,
            stdout: stdout,
        }
    }

    pub fn expand_globs(&mut self) {
        let jobs = self.jobs.drain(..).map(|mut job| {
            job.expand_globs();
            job
        }).collect();
        self.jobs = jobs;
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Job {
    pub command: String,
    pub args: Vec<String>,
    pub background: bool,
}

impl Job {

    pub fn new(args: Vec<String>, background: bool) -> Self {
        let command = args[0].clone();
        Job {
            command: command,
            args: args,
            background: background,
        }
    }

    pub fn expand_globs(&mut self) {
        let mut new_args: Vec<String> = vec![];
        for arg in self.args.drain(..) {
            let mut pushed_glob = false;
            if arg.contains(|chr| chr == '?' || chr == '*' || chr == '[') {
                if let Ok(expanded) = glob(&arg) {
                    for path in expanded.filter_map(Result::ok) {
                        pushed_glob = true;
                        new_args.push(path.to_string_lossy().into_owned());
                    }
                }
            }
            if !pushed_glob {
                new_args.push(arg);
            }
        }
        self.args = new_args;
    }

    pub fn build_command(&self) -> Command {
        let mut command = Command::new(&self.command);
        for i in 1..self.args.len() {
            if let Some(arg) = self.args.get(i) {
                command.arg(arg);
            }
        }
        command
    }
}

pub fn parse(code: &str) -> Vec<Pipeline> {
    match pipelines(code) {
		Ok(code_ok) => code_ok,
		Err(err) => {
			println!("ion: Syntax {}",err);
			vec![]
		}
	}
}

peg! grammar(r#"
use super::Pipeline;
use super::Job;
use super::Redirection;
use super::IfStatement;
use super::ShellUpdate;
use super::Comparitor::*;
use super::Comparitor;


#[pub]
if_ -> ShellUpdate
    = "if " l:_not_comparitor whitespace c:comparitor whitespace r:_not_comparitor { ShellUpdate::FlowControl(IfStatement{ left: l, comparitor: c, right: r}) }

comparitor -> Comparitor
    = "==" { Equal }
    / "!=" { NotEqual }
    / "<"  { LessThan }
    / "<=" { LessThanOrEqual }
    / ">"  { GreaterThan }
    / ">=" { GreaterThanOrEqual }

_not_comparitor -> String
    = !comparitor [^ ]+ { match_str.to_string() }

#[pub]
pipelines -> Vec<Pipeline>
    = (unused* newline)* pipelines:pipeline ++ ((job_ending+ unused*)+) (newline unused*)* { pipelines }
    / (unused*) ** newline { vec![] }

pipeline -> Pipeline
    = whitespace? res:job ++ pipeline_sep whitespace? redir:redirection whitespace? comment? { Pipeline::new(res, redir.0, redir.1) }

job -> Job
    = args:word ++ whitespace background:background_token? {
        Job::new(args.iter().map(|arg|arg.to_string()).collect(), background.is_some())
    }

redirection -> (Option<Redirection>, Option<Redirection>)
    = stdin:redirect_stdin whitespace? stdout:redirect_stdout? { (Some(stdin), stdout) }
    / stdout:redirect_stdout whitespace? stdin:redirect_stdin? { (stdin, Some(stdout)) }
    / { (None, None) }

redirect_stdin -> Redirection
    = [<] whitespace? file:word { Redirection { file: file.to_string(), append: false } }

redirect_stdout -> Redirection
    = [>]{2} whitespace? file:word { Redirection { file: file.to_string(), append: true } }
    / [>] whitespace? file:word { Redirection { file: file.to_string(), append: false } }

pipeline_sep -> ()
    = (whitespace? [|] whitespace?) { }

background_token -> ()
    = [&]
    / whitespace [&]

word -> &'input str
    = double_quoted_word
    / single_quoted_word
    / [^ \t\r\n#;&|<>]+ { match_str }

double_quoted_word -> &'input str
    = ["] word:_double_quoted_word ["] { word }

_double_quoted_word -> &'input str
    = [^"]+ { match_str }

single_quoted_word -> &'input str
    = ['] word:_single_quoted_word ['] { word }

_single_quoted_word -> &'input str
    = [^']+ { match_str }

unused -> ()
    = whitespace comment? { () }
    / comment { () }

comment -> ()
    = [#] [^\r\n]*

whitespace -> ()
    = [ \t]+

job_ending -> ()
    = [;]
    / newline
    / newline

newline -> ()
    = [\r\n]
"#);


#[cfg(test)]
mod tests {
    use super::*;
    use super::grammar::*;

    #[test]
    fn single_job_no_args() {
        let jobs = parse("cat").remove(0).jobs;
        assert_eq!(1, jobs.len());
        assert_eq!("cat", jobs[0].command);
        assert_eq!(1, jobs[0].args.len());
    }

    #[test]
    fn single_job_with_args() {
        let jobs = parse("ls -al dir").remove(0).jobs;
        assert_eq!(1, jobs.len());
        assert_eq!("ls", jobs[0].command);
        assert_eq!("-al", jobs[0].args[1]);
        assert_eq!("dir", jobs[0].args[2]);
    }

    #[test]
    fn multiple_jobs_with_args() {
        let pipelines = parse("ls -al;cat tmp.txt");
        assert_eq!(2, pipelines.len());
        assert_eq!("ls", pipelines[0].jobs[0].command);
        assert_eq!("-al", pipelines[0].jobs[0].args[1]);
        assert_eq!("cat", pipelines[1].jobs[0].command);
        assert_eq!("tmp.txt", pipelines[1].jobs[0].args[1]);
    }

    #[test]
    fn parse_empty_string() {
        let pipelines = parse("");
        assert_eq!(0, pipelines.len());
    }

    #[test]
    fn multiple_white_space_between_words() {
        let jobs = parse("ls \t -al\t\tdir").remove(0).jobs;
        assert_eq!(1, jobs.len());
        assert_eq!("ls", jobs[0].command);
        assert_eq!("-al", jobs[0].args[1]);
        assert_eq!("dir", jobs[0].args[2]);
    }

    #[test]
    fn trailing_whitespace() {
        let pipelines = parse("ls -al\t ");
        assert_eq!(1, pipelines.len());
        assert_eq!("ls", pipelines[0].jobs[0].command);
        assert_eq!("-al", pipelines[0].jobs[0].args[1]);
    }

    #[test]
    fn double_quoting() {
        let jobs = parse("echo \"Hello World\"").remove(0).jobs;
        assert_eq!(2, jobs[0].args.len());
        assert_eq!("Hello World", jobs[0].args[1]);
    }

    #[test]
    fn all_whitespace() {
        let pipelines = parse("  \t ");
        assert_eq!(0, pipelines.len());
    }

    #[test]
    fn not_background_job() {
        let jobs = parse("echo hello world").remove(0).jobs;
        assert_eq!(false, jobs[0].background);
    }

    #[test]
    fn background_job() {
        let jobs = parse("echo hello world&").remove(0).jobs;
        assert_eq!(true, jobs[0].background);
    }

    #[test]
    fn background_job_with_space() {
        let jobs = parse("echo hello world &").remove(0).jobs;
        assert_eq!(true, jobs[0].background);
    }

    #[test]
    fn lone_comment() {
        let pipelines = parse("# ; \t as!!+dfa");
        assert_eq!(0, pipelines.len());
    }

    #[test]
    fn command_followed_by_comment() {
        let pipelines = parse("cat # ; \t as!!+dfa");
        assert_eq!(1, pipelines.len());
        assert_eq!(1, pipelines[0].jobs[0].args.len());
    }

    #[test]
    fn comments_in_multiline_script() {
        let pipelines = parse("echo\n# a comment;\necho#asfasdf");
        assert_eq!(2, pipelines.len());
    }

    #[test]
    fn multiple_newlines() {
        let pipelines = parse("echo\n\ncat");
        assert_eq!(2, pipelines.len());
    }

    #[test]
    fn leading_whitespace() {
        let jobs = parse("    \techo").remove(0).jobs;
        assert_eq!(1, jobs.len());
        assert_eq!("echo", jobs[0].command);
    }

    #[test]
    fn indentation_on_multiple_lines() {
        let pipelines = parse("echo\n  cat");
        assert_eq!(2, pipelines.len());
        assert_eq!("echo", pipelines[0].jobs[0].command);
        assert_eq!("cat", pipelines[1].jobs[0].command);
    }

    #[test]
    fn single_quoting() {
        let jobs = parse("echo '#!!;\"\\'").remove(0).jobs;
        assert_eq!("#!!;\"\\", jobs[0].args[1]);
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        let jobs = parse("echo '#!!;\"\\' and \t some \"more' 'stuff\"").remove(0).jobs;
        assert_eq!("#!!;\"\\", jobs[0].args[1]);
        assert_eq!("and", jobs[0].args[2]);
        assert_eq!("some", jobs[0].args[3]);
        assert_eq!("more' 'stuff", jobs[0].args[4]);
    }

    #[test]
    fn several_blank_lines() {
        let pipelines = parse("\n\n\n");
        assert_eq!(0, pipelines.len());
    }

    #[test]
    fn pipelines_with_redirection() {
        let pipelines = parse("cat | echo hello | cat < stuff > other");
        assert_eq!(3, pipelines[0].jobs.len());
        assert_eq!("stuff", &pipelines[0].clone().stdin.unwrap().file);
        assert_eq!("other", &pipelines[0].clone().stdout.unwrap().file);
        assert!(!pipelines[0].clone().stdout.unwrap().append);
    }

    #[test]
    fn pipeline_with_redirection_append() {
        let pipelines = parse("cat | echo hello | cat < stuff >> other");
        assert_eq!(3, pipelines[0].jobs.len());
        assert_eq!("stuff", &pipelines[0].clone().stdin.unwrap().file);
        assert_eq!("other", &pipelines[0].clone().stdout.unwrap().file);
        assert!(pipelines[0].clone().stdout.unwrap().append);
    }

    #[test]
    fn pipelines_with_redirection_reverse_order() {
        let pipelines = parse("cat | echo hello | cat > stuff < other");
        assert_eq!(3, pipelines[0].jobs.len());
        assert_eq!("other", &pipelines[0].clone().stdin.unwrap().file);
        assert_eq!("stuff", &pipelines[0].clone().stdout.unwrap().file);
    }

    #[test]
    fn full_script() {
        pipelines(r#"if a == a
  echo true a == a

  if b != b
    echo true b != b
  else
    echo false b != b

    if 3 > 2
      echo true 3 > 2
    else
      echo false 3 > 2
    fi
  fi
else
  echo false a == a
fi
"#)
            .unwrap();  // Make sure it parses
    }

    #[test]
    fn leading_and_trailing_junk() {
        pipelines(r#"

# comment
   # comment


    if a == a
  echo true a == a  # Line ending commment

  if b != b
    echo true b != b
  else
    echo false b != b

    if 3 > 2
      echo true 3 > 2
    else
      echo false 3 > 2
    fi
  fi
else
  echo false a == a
      fi

# comment

"#).unwrap();  // Make sure it parses
    }
    #[test]
    fn parsing_ifs() {
        let parsed_if = if_("if 1 == 2").unwrap();
        let correct_parse = ShellUpdate::FlowControl(IfStatement{left: "1".to_string(),
                                        comparitor: Comparitor::Equal,
                                        right: "2".to_string()});
        assert_eq!(correct_parse, parsed_if);
    }
}
