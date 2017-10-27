use errno::errno;
use libc::*;
use shell::Shell;
use shell::foreground::ForegroundSignals;
use shell::job_control::*;
use shell::status::{FAILURE, TERMINATED};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

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

const FIRST: u8 = 1;
const LAST: u8 = 2;

pub(crate) fn watch_foreground<F, D>(
    shell: &mut Shell,
    first_pid: u32,
    last_pid: u32,
    get_command: F,
    mut drop_command: D,
) -> i32
    where F: FnOnce() -> String,
          D: FnMut(i32)
{
    let mut exit_status = 0;
    let mut found = 0;
    loop {
        unsafe {
            let mut status = 0;
            let pid = waitpid(-1, &mut status, WUNTRACED);
            match pid {
                -1 => {
                    let error = errno();
                    match error.0 {
                        ECHILD => break exit_status,
                        _ => {
                            eprintln!("ion: {}", error);
                            break FAILURE;
                        }
                    }
                }
                0 => (),
                _ if WIFEXITED(status) => {
                    let status = WEXITSTATUS(status) as i32;
                    if pid == (last_pid as i32) {
                        found |= LAST;
                    }

                    if pid == (first_pid as i32) {
                        found |= FIRST;
                    }

                    if found == FIRST + LAST {
                        break status;
                    } else {
                        drop_command(pid);
                        exit_status = status;
                    }
                }
                _ if WIFSIGNALED(status) => {
                    eprintln!("ion: process ended by signal");
                    let signal = WTERMSIG(status);
                    match signal {
                        SIGINT => {
                            shell.foreground_send(signal as i32);
                            shell.break_flow = true;
                        }
                        _ => {
                            shell.handle_signal(signal);
                            shell.exit(TERMINATED);
                        }
                    }
                    break TERMINATED;
                }
                _ if WIFSTOPPED(status) => {
                    shell.send_to_background(pid as u32, ProcessState::Stopped, get_command());
                    shell.break_flow = true;
                    break TERMINATED;
                }
                _ => (),
            }
        }
    }
}
