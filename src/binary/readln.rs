use super::{completer::IonCompleter, InteractiveShell};
use ion_shell::Shell;
use std::io::ErrorKind;

impl<'a> InteractiveShell<'a> {
    /// Ion's interface to Liner's `read_line` method, which handles everything related to
    /// rendering, controlling, and getting input from the prompt.
    pub fn readln<T: Fn(&mut Shell<'_>)>(&self, prep_for_exit: &T) -> Option<String> {
        let prompt = self.prompt();
        let line = self.context.borrow_mut().read_line(
            prompt,
            None,
            &mut IonCompleter::new(&self.shell.borrow()),
        );

        match line {
            Ok(line) => {
                if line.bytes().next() != Some(b'#')
                    && line.bytes().any(|c| !c.is_ascii_whitespace())
                {
                    self.shell.borrow_mut().unterminated = true;
                }
                Some(line)
            }
            // Handles Ctrl + C
            Err(ref err) if err.kind() == ErrorKind::Interrupted => None,
            // Handles Ctrl + D
            Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => {
                let mut shell = self.shell.borrow_mut();
                if !shell.unterminated && shell.exit_block().is_err() {
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
