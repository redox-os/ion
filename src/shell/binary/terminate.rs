use super::super::{Binary, FlowLogic, Shell};
use super::super::status::*;
use parser::Terminator;

pub(crate) fn terminate_script_quotes<I: Iterator<Item = String>>(
    shell: &mut Shell,
    mut lines: I,
) -> i32 {
    while let Some(command) = lines.next() {
        let mut buffer = Terminator::new(command);
        while !buffer.is_terminated() {
            loop {
                if let Some(command) = lines.next() {
                    if !command.starts_with('#') {
                        let mut start = 0;
                        let cmd: &str = loop {
                            if start >= command.len() {
                                break &command;
                            }

                            match command[start..].find('#').map(|x| x + start) {
                                Some(pos) if command.as_bytes()[pos-1] != b' ' => {
                                    start = pos + 1;
                                }
                                Some(pos) => {
                                    break &command[..pos]
                                }
                                None => break &command
                            }
                        };
                        buffer.append(cmd);
                        break;
                    }
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
    let mut buffer = Terminator::new(command);
    shell.flow_control.level += 1;
    while !buffer.is_terminated() {
        if let Some(command) = shell.readln() {
            if !command.starts_with('#') {
                buffer.append(&command);
            }
        } else {
            return Err(());
        }
    }
    shell.flow_control.level -= 1;
    let terminated = buffer.consume();
    Ok(terminated)
}
