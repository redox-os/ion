use shell::{flags::UNTERMINATED, status::*, Binary, FlowLogic, Shell};
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
                                Some(pos) if command.as_bytes()[pos - 1] != b' ' => {
                                    start = pos + 1;
                                }
                                Some(pos) => break &command[..pos],
                                None => break &command,
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

    if shell.flow_control.unclosed_block() {
        let open_block = shell.flow_control.block.last().unwrap();
        eprintln!(
            "ion: unexpected end of script: expected end block for `{}`",
            open_block.short(),
        );
        return FAILURE;
    }

    SUCCESS
}

pub(crate) fn terminate_quotes(shell: &mut Shell, command: String) -> Result<String, ()> {
    let mut buffer = Terminator::new(command);
    shell.flags |= UNTERMINATED;
    while ! buffer.is_terminated() {
        if let Some(command) = shell.readln() {
            if ! command.starts_with('#') {
                buffer.append(&command);
            }
        } else {
            shell.flags ^= UNTERMINATED;
            return Err(());
        }
    }

    shell.flags ^= UNTERMINATED;
    let terminated = buffer.consume();
    Ok(terminated)
}
