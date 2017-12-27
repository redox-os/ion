use std::error::Error;
use std::io::{Write, stdout};
use std::process::Command;
use std::time::Instant;

const MAN_PAGE: &'static str = r#"NAME
    time - timer for commands

SYNOPSIS
    time [ -h | --help ][COMMAND] [ARGUEMENT]...

DESCRIPTION
    Runs the command taken as the first arguement and outputs the time the command took to execute.

OPTIONS
    -h
    --help
        display this help and exit
"#;

pub(crate) fn time(args: &[&str]) -> Result<(), String> {
    let stdout = stdout();
    let mut stdout = stdout.lock();

    for arg in args {
        if *arg == "-h" || *arg == "--help" {
            return match stdout.write_all(MAN_PAGE.as_bytes()).and_then(
                |_| stdout.flush(),
            ) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.description().to_owned()),
            };
        }
    }

    let time = Instant::now();

    if !args.is_empty() {
        let mut command = Command::new(&args[0]);
        for arg in &args[1..] {
            command.arg(arg);
        }
        command.spawn().and_then(|mut child| child.wait()).map_err(
            |err| {
                format!("time: {:?}", err)
            },
        )?;
    }

    let duration = time.elapsed();
    let seconds = duration.as_secs();
    let nanoseconds = duration.subsec_nanos();

    if seconds > 60 {
        write!(stdout, "real    {}m{:02}.{:09}s\n", seconds / 60, seconds % 60, nanoseconds)
            .map_err(|x| x.description().to_owned())?;
    } else {
        write!(stdout, "real    {}.{:09}s\n", seconds, nanoseconds)
            .map_err(|x| x.description().to_owned())?;
    }

    Ok(())
}
