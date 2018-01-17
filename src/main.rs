extern crate arg_parser;
extern crate ion_shell;
extern crate smallvec;

use arg_parser::ArgParser;
use ion_shell::{flags::NO_EXEC, Binary, JobControl, ShellBuilder, MAN_ION};
use smallvec::SmallVec;
use std::{
    env, error::Error, io::{stdout, Write}, iter::FromIterator,
};

fn main() {
    let mut shell = ShellBuilder::new()
        .install_signal_handler()
        .block_signals()
        .set_unique_pid()
        .as_binary();

    let mut args = ArgParser::new(4)
        .add_flag(&["h", "help"])
        .add_flag(&["n", "no-execute"])
        .add_flag(&["v", "version"])
        .add_opt("c", "command");

    args.parse(env::args());

    if args.found("no-execute") {
        shell.flags |= NO_EXEC;
    }

    if args.found("help") {
        let stdout = stdout();
        let mut stdout = stdout.lock();
        stdout
            .write_all(MAN_ION.as_bytes())
            .and_then(|_| stdout.flush())
            .unwrap();
    } else if args.found("version") {
        shell.display_version();
    } else if let Some(script) = args.get_opt("command") {
        if let Err(err) = shell.execute_command(script) {
            eprintln!("ion: failed to execute command: {}", err);
        }
    } else {
        let mut drain = args.args.drain(..);
        if let Some(path) = drain.next() {
            let mut array = SmallVec::from_iter(Some(path.clone().into()));
            for arg in drain {
                array.push(arg.into());
            }
            shell.variables.set_array("args", array);
            if let Err(err) = shell.execute_script(&path) {
                eprintln!("ion: {}", err);
            }

            shell.wait_for_background();
            let previous_status = shell.previous_status;
            shell.exit(previous_status);
        } else {
            shell.execute_interactive();
        }
    }
}
