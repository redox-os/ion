use glob::glob;

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

    pub fn expand_globs(&mut self) {
        let mut new_args: Vec<String> = vec![];
        for arg in self.args.drain(..) {
            let mut pushed_glob = false;
            if let Ok(expanded) = glob(&arg) {
                for path in expanded.filter_map(Result::ok) {
                    pushed_glob = true;
                    new_args.push(path.to_string_lossy().into_owned());
                }
            } 
            if !pushed_glob {
                new_args.push(arg);
            }
        }
        self.args = new_args;
    }
}
