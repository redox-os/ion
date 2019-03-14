use crate::{
    parser::Terminator,
    shell::{flags::UNTERMINATED, status::*, Binary, FlowLogic, Shell},
};

pub(crate) fn terminate_script_quotes<T: AsRef<str> + ToString, I: Iterator<Item = T>>(
    shell: &mut Shell,
    mut lines: I,
) -> i32 {
    while let Some(command) = lines.next() {
        let mut buffer = Terminator::new(command.to_string());
        while !buffer.is_terminated() {
            if let Some(command) = lines.find(|cmd| !cmd.as_ref().starts_with('#')) {
                buffer.append(command.as_ref().splitn(2, " #").next().unwrap());
            } else {
                eprintln!("ion: unterminated quote in script");
                return FAILURE;
            }
        }
        shell.on_command(&buffer.consume());
    }

    if shell.flow_control.unclosed_block() {
        let open_block = shell.flow_control.block.last().unwrap();
        eprintln!("ion: unexpected end of script: expected end block for `{}`", open_block.short(),);
        FAILURE
    } else {
        SUCCESS
    }
}

pub(crate) fn terminate_quotes(shell: &mut Shell, command: String) -> Result<String, ()> {
    let mut buffer = Terminator::new(command);
    shell.flags |= UNTERMINATED;
    while !buffer.is_terminated() {
        if let Some(command) = shell.readln() {
            if !command.starts_with('#') {
                buffer.append(&command);
            }
        } else {
            shell.flags ^= UNTERMINATED;
            return Err(());
        }
    }

    shell.flags ^= UNTERMINATED;
    Ok(buffer.consume())
}
