use std::process::{Stdio, Command, Child};
use std::os::unix::io::{FromRawFd, IntoRawFd};

pub fn pipe(commands: &mut [Command]) {
    if commands.len() == 0 {
        return;
    }
    let end = commands.len() - 1;
    for command in &mut commands[..end] {
        command.stdout(Stdio::piped());
    }
    let mut prev: Option<Child> = None;
    for command in commands {
        if let Some(child) = prev {
            unsafe {
                command.stdin(Stdio::from_raw_fd(child.stdout.unwrap().into_raw_fd()));
            }
        }
        prev = Some(command.spawn().expect(""));
    }
}
