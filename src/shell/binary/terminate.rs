use super::super::{Binary, FlowLogic, Shell};
use super::super::status::*;
use parser::QuoteTerminator;

pub(crate) fn terminate_script_quotes<I: Iterator<Item = String>>(
    shell: &mut Shell,
    mut lines: I,
) -> i32 {
    while let Some(command) = lines.next() {
        let mut buffer = QuoteTerminator::new(command);
        while !buffer.check_termination() {
            loop {
                if let Some(command) = lines.next() {
                    buffer.append(command);
                    break;
                } else {
                    eprintln!("ion: unterminated quote in script");
                    return FAILURE;
                }
            }
        }
        shell.on_command(&buffer.consume());
    }

    // The flow control level being non zero means that we have a statement that has
    // only been partially parsed.
    if shell.flow_control.level != 0 {
        eprintln!(
            "ion: unexpected end of script: expected end block for `{}`",
            shell.flow_control.current_statement.short()
        );
        return FAILURE;
    }

    SUCCESS
}

pub(crate) fn terminate_quotes(shell: &mut Shell, command: String) -> Result<String, ()> {
    let mut buffer = QuoteTerminator::new(command);
    shell.flow_control.level += 1;
    while !buffer.check_termination() {
        if let Some(command) = shell.readln() {
            buffer.append(command);
        } else {
            return Err(());
        }
    }
    shell.flow_control.level -= 1;
    let terminated = buffer.consume();
    Ok(terminated)
}
