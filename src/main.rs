extern crate ion_shell;
extern crate smallvec;

use ion_shell::JobControl;
use ion_shell::MAN_ION;
use ion_shell::flags::NO_EXEC;
use ion_shell::{Binary, ShellBuilder};
use smallvec::SmallVec;
use std::env;
use std::error::Error;
use std::io::{stdout, Write};
use std::iter::FromIterator;

fn main() {
    let mut shell = ShellBuilder::new()
        .install_signal_handler()
        .block_signals()
        .set_unique_pid()
        .as_binary();

    let mut args = env::args().skip(1);
    while let Some(path) = args.next() {
        match path.as_str() {
            "-n" | "--no-execute" => {
                shell.flags |= NO_EXEC;
                continue;
            }
            // Ion currently does not do anything when invoked as a login shell, however some scripts 
            // automatically pass `-l` as an argument so we need to ignore it explicitly.
            "-l" | "--login" => continue,
            "-c" => shell.execute_arguments(args),
            "-v" | "--version" => shell.display_version(),
            "-h" | "--help" => {
                let stdout = stdout();
                let mut stdout = stdout.lock();
                match stdout
                    .write_all(MAN_ION.as_bytes())
                    .and_then(|_| stdout.flush())
                {
                    Ok(_) => return,
                    Err(err) => panic!("{}", err.description().to_owned()),
                }
            }
            _ => {
                let mut array = SmallVec::from_iter(Some(path.clone().into()));
                for arg in args {
                    array.push(arg.into());
                }
                shell.variables.set_array("args", array);
                if let Err(err) = shell.execute_script(&path) {
                    eprintln!("ion: {}", err);
                }
            }
        }

        shell.wait_for_background();
        let previous_status = shell.previous_status;
        shell.exit(previous_status);
    }

    shell.execute_interactive();
}
