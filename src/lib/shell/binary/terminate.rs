use crate::{
    parser::Terminator,
    shell::{flags::UNTERMINATED, status::*, Binary, FlowLogic, Shell},
};

pub(crate) fn terminate_script_quotes<T: AsRef<str> + ToString, I: Iterator<Item = T>>(
    shell: &mut Shell,
    lines: I,
) -> i32 {
    let mut lines = lines.filter(|cmd| !cmd.starts_with('#'));
    while let Some(command) = lines.next() {
        match Terminator::new(command).terminate(&mut lines) {
            Ok(stmt) => shell.on_command(&stmt),
            Err(_) => {
                eprintln!("ion: unterminated quote in script");
                return FAILURE;
            }
        }
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
    let buffer = Terminator::new(command);
    shell.flags |= UNTERMINATED;
    let mut lines = itertools::repeat_call(|| shell.readln()).filter_map(|cmd| cmd).filter(|cmd| !cmd.starts_with('#'));

    let stmt = buffer.terminate(&mut lines).map(|stmt| stmt.to_string());

    shell.flags &= !UNTERMINATED;

    stmt
}
