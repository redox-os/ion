use std::io::{stderr, Write};

use shell::flow_control::Statement;
use self::grammar::parse_;
use shell::directory_stack::DirectoryStack;
use shell::Job;
use shell::variables::Variables;
use parser::{expand_string, ExpanderFunctions, Index, IndexEnd};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RedirectFrom { Stdout, Stderr, Both}

#[derive(Debug, PartialEq, Clone)]
pub struct Redirection {
    pub from:   RedirectFrom,
    pub file:   String,
    pub append: bool
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pipeline {
    pub jobs:   Vec<Job>,
    pub stdout: Option<Redirection>,
    pub stdin:  Option<Redirection>,
}

impl Pipeline {
    pub fn new(jobs: Vec<Job>, stdin: Option<Redirection>, stdout: Option<Redirection>) -> Self
    {
        Pipeline {
            jobs:   jobs,
            stdin:  stdin,
            stdout: stdout
        }
    }

    pub fn expand(&mut self, variables: &Variables, dir_stack: &DirectoryStack) {
        let expanders = get_expanders!(variables, dir_stack);
        for job in &mut self.jobs {
            job.expand(&expanders);
        }

        if let Some(stdin) = self.stdin.iter_mut().next() {
            stdin.file = expand_string(stdin.file.as_str(), &expanders, false).join(" ");
        }

        if let Some(stdout) = self.stdout.iter_mut().next() {
            stdout.file = expand_string(stdout.file.as_str(), &expanders, false).join(" ");
        }
    }
}

pub fn parse(code: &str) -> Statement {
    match parse_(code) {
		Ok(code_ok) => code_ok,
		Err(err) => {
            let stderr = stderr();
            let _ = writeln!(stderr.lock(), "ion: Syntax {}", err);
			Statement::Default
		}
	}
}

peg_file! grammar("grammar.rustpeg");

#[cfg(test)]
mod tests {
    use super::grammar::*;
    use super::*;
    use shell::flow_control::Statement;
    use shell::JobKind;

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
        // Default case where spaced normally
        let parsed_if = if_("if test 1 -eq 2").unwrap();
        let correct_parse = Statement::If {
            expression: Pipeline::new(
                vec!(Job::new(
                    vec![
                        "test".to_owned(),
                        "1".to_owned(),
                        "-eq".to_owned(),
                        "2".to_owned(),
                    ].into_iter().collect(), JobKind::Last)
                ), None, None),
            success: vec!(),
            else_if: vec!(),
            failure: vec!()
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = if_("if test 1 -eq 2         ").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_elses() {
        // Default case where spaced normally
        let parsed_if = else_("else").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = else_("else         ").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = else_("         else").unwrap();
        let correct_parse = Statement::Else;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_ends() {
        // Default case where spaced normally
        let parsed_if = end_("end").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = end_("end         ").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = end_("         end").unwrap();
        let correct_parse = Statement::End;
        assert_eq!(correct_parse, parsed_if);
    }

    #[test]
    fn parsing_functions() {
        // Default case where spaced normally
        let parsed_if = fn_("fn bob").unwrap();
        let correct_parse = Statement::Function{
            description: "".into(),
            name:        "bob".into(),
            args:        Default::default(),
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = fn_("fn bob        ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = fn_("         fn bob").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Default case where spaced normally
        let parsed_if = fn_("fn bob a b").unwrap();
        let correct_parse = Statement::Function{
            description: "".into(),
            name:        "bob".into(),
            args:        vec!["a".to_owned(), "b".to_owned()],
            statements:  Default::default(),
        };
        assert_eq!(correct_parse, parsed_if);

        // Trailing spaces after final value
        let parsed_if = fn_("fn bob a b       ").unwrap();
        assert_eq!(correct_parse, parsed_if);

        // Leading spaces after final value
        let parsed_if = fn_("         fn bob a b").unwrap();
        assert_eq!(correct_parse, parsed_if);

        let parsed_if = fn_("fn bob a b --bob is a nice function").unwrap();
        let correct_parse = Statement::Function{
            description: "bob is a nice function".to_string(),
            name:        "bob".into(),
            args:        vec!("a".to_owned(), "b".to_owned()),
            statements:  vec!()
        };
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = fn_("fn bob a b --          bob is a nice function").unwrap();
        assert_eq!(correct_parse, parsed_if);
        let parsed_if = fn_("fn bob a b      --bob is a nice function").unwrap();
        assert_eq!(correct_parse, parsed_if);
    }
}
