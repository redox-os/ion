use shell::{
    foreground::ForegroundSignals, job_control::*, status::{FAILURE, TERMINATED}, Shell,
};
use std::{
    sync::{Arc, Mutex}, thread::sleep, time::Duration,
};
use syscall::{
    kill, waitpid, wcoredump, wexitstatus, wifcontinued, wifexited, wifsignaled, wifstopped,
    wstopsig, wtermsig, ECHILD, SIGINT, SIGPIPE, WCONTINUED, WNOHANG, WUNTRACED,
};

const OPTS: usize = WUNTRACED | WCONTINUED | WNOHANG;

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
            match waitpid(-(pgid as isize) as usize, &mut status, OPTS) {
                Err(ref err) if err.errno == ECHILD => {
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
                Err(err) => {
                    eprintln!("ion: ([{}] {}) errored: {}", njob, pgid, err);
                    let mut processes = processes.lock().unwrap();
                    let process = &mut processes.iter_mut().nth(njob).unwrap();
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.errored();
                    }
                    break;
                }
                Ok(0) => (),
                Ok(_pid) if wifexited(status) => exit_status = wexitstatus(status),
                Ok(_pid) if wifstopped(status) => {
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
                Ok(_pid) if wifcontinued(status) => {
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
            match waitpid(pid as usize, &mut status, WUNTRACED) {
                Err(err) => match err.errno {
                    ECHILD if signaled == 0 => break exit_status,
                    ECHILD => break signaled,
                    errno => {
                        eprintln!("ion: waitpid error: {}", errno);
                        break FAILURE;
                    }
                },
                Ok(0) => (),
                Ok(_pid) if wifexited(status) => exit_status = wexitstatus(status) as i32,
                Ok(pid) if wifsignaled(status) => {
                    let signal = wtermsig(status);
                    if signal == SIGPIPE {
                        continue;
                    } else if wcoredump(status) {
                        eprintln!("ion: process ({}) had a core dump", pid);
                        continue;
                    }

                    eprintln!("ion: process ({}) ended by signal {}", pid, signal);
                    match signal {
                        SIGINT => {
                            let _ = kill(pid, signal as usize);
                            shell.break_flow = true;
                        }
                        _ => {
                            shell.handle_signal(signal as i32);
                        }
                    }
                    signaled = 128 + signal as i32;
                }
                Ok(pid) if wifstopped(status) => {
                    shell.send_to_background(pid as u32, ProcessState::Stopped, command.into());
                    shell.break_flow = true;
                    break 128 + wstopsig(status) as i32;
                }
                _ => (),
            }
        }
    }
}
