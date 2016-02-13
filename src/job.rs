#[derive(Debug, PartialEq)]
pub struct Job {
    pub command: String,
    pub args: Vec<String>,
}

impl Job {
    pub fn new(args: Vec<&str>) -> Self {
        let command = args[0].to_string();
        let args = args.iter().map(|arg| arg.to_string()).collect();
        Job {
            command: command,
            args: args,
        }
    }

    pub fn from_vec_string(args: Vec<String>) -> Self {
        let command = args[0].clone();
        Job {
            command: command,
            args: args,
        }
    }
}
