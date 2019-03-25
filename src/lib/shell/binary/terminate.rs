use crate::{
    parser::Terminator,
    shell::{status::*, FlowLogic, Shell},
};
use itertools::Itertools;

pub(crate) fn terminate_script_quotes<I: Iterator<Item = u8>>(shell: &mut Shell, lines: I) -> i32 {
    for cmd in lines.batching(|lines| Terminator::new(lines).terminate()) {
        match cmd {
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
