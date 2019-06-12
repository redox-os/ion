use super::{completer::IonCompleter, InteractiveBinary};
use std::io::ErrorKind;

pub fn readln(binary: &InteractiveBinary<'_>) -> Option<String> {
    let prompt = binary.prompt();
    let line = binary.context.borrow_mut().read_line(
        prompt,
        None,
        &mut IonCompleter::new(&binary.shell.borrow()),
    );

    match line {
        Ok(line) => {
            if line.bytes().next() != Some(b'#') && line.bytes().any(|c| !c.is_ascii_whitespace()) {
                binary.shell.borrow_mut().unterminated = true;
            }
            Some(line)
        }
        // Handles Ctrl + C
        Err(ref err) if err.kind() == ErrorKind::Interrupted => None,
        // Handles Ctrl + D
        Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => {
            let mut shell = binary.shell.borrow_mut();
            if !shell.unterminated && shell.exit_block().is_err() {
                shell.prep_for_exit();
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
