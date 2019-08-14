use super::{completer::IonCompleter, InteractiveShell};
use rustyline::{error::ReadlineError, Editor};

impl InteractiveShell {
    /// Ion's interface to Liner's `read_line` method, which handles everything related to
    /// rendering, controlling, and getting input from the prompt.
    pub fn readln(&self, context: &mut Editor<IonCompleter<'_, '_>>) -> Option<String> {
        let prompt = {
            let mut shell = context.helper_mut().unwrap().shell().borrow_mut();
            self.prompt(&mut shell)
        };
        let line = context.readline(&prompt);

        match line {
            Ok(line) => {
                if line.bytes().next() != Some(b'#')
                    && line.bytes().any(|c| !c.is_ascii_whitespace())
                {
                    self.terminated.set(false);
                }
                Some(line)
            }
            // Handles Ctrl + C
            Err(ReadlineError::Interrupted) => None,
            // Handles Ctrl + D
            Err(ReadlineError::Eof) => {
                let mut shell = context.helper_mut().unwrap().shell().borrow_mut();
                if self.terminated.get() && shell.exit_block().is_err() {
                    let prep = shell.builtins().get("exit").unwrap();
                    prep(&["exit".into()], &mut shell);
                    std::process::exit(shell.previous_status().as_os_code())
                }
                None
            }
            Err(err) => {
                eprintln!("ion: liner: {}", err);
                None
            }
        }
    }
}
