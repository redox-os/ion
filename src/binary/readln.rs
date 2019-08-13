use super::InteractiveShell;
use ion_shell::Shell;
use rustyline::error::ReadlineError;

impl<'a> InteractiveShell<'a> {
    /// Ion's interface to Liner's `read_line` method, which handles everything related to
    /// rendering, controlling, and getting input from the prompt.
    pub fn readln<T: Fn(&mut Shell<'_>)>(&self, prep_for_exit: &T) -> Option<String> {
        let line = self.context.borrow_mut().readline(&self.prompt());

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
                let mut shell = self.shell.borrow_mut();
                if self.terminated.get() && shell.exit_block().is_err() {
                    prep_for_exit(&mut shell);
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
