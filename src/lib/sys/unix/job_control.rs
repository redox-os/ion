use super::{errno, write_errno};
use libc::*;
use shell::{
    foreground::ForegroundSignals,
    job_control::*,
    status::{FAILURE, TERMINATED},
    Shell,
};
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

const OPTS: i32 = WUNTRACED | WCONTINUED | WNOHANG;

pub(crate) fn watch_background(
    fg: Arc<ForegroundSignals>,
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pgid: u32,
    njob: usize,
) {
    let mut fg_was_grabbed = false;
    let mut status;
    let mut exit_status = 0;

    loop {
        fg_was_grabbed = !fg_was_grabbed && fg.was_grabbed(pgid);

        unsafe {
            status = 0;
            match waitpid(-(pgid as pid_t), &mut status, OPTS) {
                -1 if errno() == ECHILD => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) exited with {}", njob, pgid, status);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.reply_with(exit_status as i8);
                    }
                    break;
                }
                -1 => {
                    eprintln!("ion: ([{}] {}) errored: {}", njob, pgid, errno());
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.errored();
                    }
                    break;
                }
                0 => (),
                _pid if WIFEXITED(status) => exit_status = WEXITSTATUS(status),
                _pid if WIFSTOPPED(status) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) Stopped", njob, pgid);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    if fg_was_grabbed {
                        fg.reply_with(TERMINATED as i8);
                        fg_was_grabbed = false;
                    }
                    process.state = ProcessState::Stopped;
                }
                _pid if WIFCONTINUED(status) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) Running", njob, pgid);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Running;
                }
                _ => (),
            }
        }
        sleep(Duration::from_millis(100));
    }
}

pub(crate) fn watch_foreground(shell: &mut Shell, pid: i32, command: &str) -> i32 {
    let mut signaled = 0;
    let mut exit_status = 0;
    let mut status;

    loop {
        unsafe {
            status = 0;
            match waitpid(pid, &mut status, WUNTRACED) {
                -1 => match errno() {
                    ECHILD if signaled == 0 => break exit_status,
                    ECHILD => break signaled,
                    errno => {
                        write_errno("ion: waitpid error: ", errno);
                        break FAILURE;
                    }
                },
                0 => (),
                _pid if WIFEXITED(status) => exit_status = WEXITSTATUS(status),
                pid if WIFSIGNALED(status) => {
                    let signal = WTERMSIG(status);
                    if signal == SIGPIPE {
                        continue;
                    } else if WCOREDUMP(status) {
                        eprintln!("ion: process ({}) had a core dump", pid);
                        continue;
                    }

                    eprintln!("ion: process ({}) ended by signal {}", pid, signal);
                    match signal {
                        SIGINT => {
                            let _ = kill(pid, signal as i32);
                            shell.break_flow = true;
                        }
                        _ => {
                            shell.handle_signal(signal);
                        }
                    }
                    signaled = 128 + signal as i32;
                }
                pid if WIFSTOPPED(status) => {
                    shell.send_to_background(
                        pid.abs() as u32,
                        ProcessState::Stopped,
                        command.into(),
                    );
                    shell.break_flow = true;
                    break 128 + WSTOPSIG(status);
                }
                _ => (),
            }
        }
    }
}
