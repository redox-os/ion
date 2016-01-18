use super::tokenizer::Token;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Job {
    pub command: String,
    pub args: Vec<String>,
}

//impl Job {
//}

pub fn parse(tokens: &mut Vec<Token>) -> Vec<Job> {
    let mut jobs: Vec<Job> = vec![];
    let mut job = Job::default();
    for token in tokens.drain(..) {
        match token {
            Token::Word(word) => {
                if job.command.is_empty() {
                    job.command = word;
                } else {
                    job.args.push(word);
                }
            }
            Token::End => {
                if !job.command.is_empty() {
                    jobs.push(job);
                    job = Job::default();
                }
            }
        }
    }
    jobs
}

#[cfg(test)]
mod tests {

    use super::super::tokenizer::Token;
    use super::*;

    #[test]
    fn parse_end_token() {
        let mut tokens: Vec<Token> = vec![Token::End];
        let expected: Vec<Job> = vec![];
        assert_eq!(expected, parse(&mut tokens));
    }

    #[test]
    fn parse_command_no_args() {
        let mut tokens: Vec<Token> = vec![Token::Word("ls".to_string()), Token::End];
        let expected = vec![Job {
                                command: "ls".to_string(),
                                args: vec![],
                            }];
        assert_eq!(expected, parse(&mut tokens));
    }

    #[test]
    fn parse_command_with_args() {
        let mut tokens: Vec<Token> = vec![Token::Word("ls".to_string()),
                                          Token::Word("-a".to_string()),
                                          Token::Word("-l".to_string()),
                                          Token::End];
        let expected = vec![Job {
                                command: "ls".to_string(),
                                args: vec!["-a".to_string(), "-l".to_string()],
                            }];
        assert_eq!(expected, parse(&mut tokens));
    }

    #[test]
    fn parse_multiple_jobs() {
        let mut tokens: Vec<Token> = vec![
            Token::Word("ls".to_string()),
            Token::Word("-a".to_string()),
            Token::Word("-l".to_string()),
            Token::End,
            Token::Word("echo".to_string()),
            Token::Word("hello world".to_string()),
            Token::End,
        ];
        let expected = vec![Job {
                                command: "ls".to_string(),
                                args: vec!["-a".to_string(), "-l".to_string()],
                            },
                            Job {
                                command: "echo".to_string(),
                                args: vec!["hello world".to_string()],
                            }];
        assert_eq!(expected, parse(&mut tokens));
    }
}
