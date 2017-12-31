use libc::*;
use shell::Shell;
use shell::foreground::ForegroundSignals;
use shell::job_control::*;
use shell::status::{FAILURE, TERMINATED};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use super::{errno, write_errno};

pub(crate) fn watch_background(
    fg: Arc<ForegroundSignals>,
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    njob: usize,
) {
    let mut fg_was_grabbed = false;
    loop {
        if !fg_was_grabbed {
            if fg.was_grabbed(pid) {
                fg_was_grabbed = true;
            }
        }

        let opts = WUNTRACED | WCONTINUED | WNOHANG;
        let mut status = 0;

        unsafe {
            let pid = waitpid(-(pid as pid_t), &mut status, opts);
            match pid {
                -1 => {
                    eprintln!("ion: ([{}] {}) errored: {}", njob, pid, errno());
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.errored();
                    }
                    break;
                }
                0 => (),
                _ if WIFEXITED(status) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) exited with {}", njob, pid, status);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.reply_with(WEXITSTATUS(status) as i8);
                    }
                    break;
                }
                _ if WIFSTOPPED(status) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) Stopped", njob, pid);
                    }
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    if fg_was_grabbed {
                        fg.reply_with(TERMINATED as i8);
                        fg_was_grabbed = false;
                    }
                    process.state = ProcessState::Stopped;
                }
                _ if WIFCONTINUED(status) => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) Running", njob, pid);
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
                -1 => {
                    match errno() {
                        ECHILD if signaled == 0 => break exit_status,
                        ECHILD => break signaled,
                        errno => {
                            write_errno("ion: waitpid error: ", errno);
                            break FAILURE;
                        }
                    }
                }
                0 => (),
                _pid if WIFEXITED(status) => {
                    exit_status = WEXITSTATUS(status) as i32;
                }
                _pid if WIFSIGNALED(status) => {
                    let signal = WTERMSIG(status);
                    if signal == SIGPIPE { continue }
                    eprintln!("ion: process ended by signal {}", signal);
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
                _pid if WIFSTOPPED(status) => {
                    // TODO: Rework background control
                    shell.send_to_background(pid as u32, ProcessState::Stopped, command.into());
                    shell.break_flow = true;
                    break 128 + signal as i32;
                }
                _ => (),
            }
        }
    }
}
