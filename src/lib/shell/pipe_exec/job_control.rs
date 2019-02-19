use super::{
    super::{signals, status::*, Shell},
    foreground::{BackgroundResult, ForegroundSignals},
};
use crate::sys;
use std::{
    fmt, process,
    sync::{Arc, Mutex},
    thread::{sleep, spawn},
    time::Duration,
};

/// When given a process ID, that process's group will be assigned as the
/// foreground process group.
pub(crate) fn set_foreground_as(pid: u32) {
    signals::block();
    let _ = sys::tcsetpgrp(0, pid);
    signals::unblock();
}

pub trait JobControl {
    /// Waits for background jobs to finish before returning.
    fn wait_for_background(&mut self);
    /// Takes a background tasks's PID and whether or not it needs to be continued; resumes the
    /// task
    /// and sets it as the foreground process. Once the task exits or stops, the exit status
    /// will
    /// be returned, and ownership of the TTY given back to the shell.
    fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> i32;
    fn resume_stopped(&mut self);
    fn handle_signal(&self, signal: i32) -> bool;
    fn background_send(&self, signal: i32);
    fn watch_foreground(&mut self, pid: i32, command: &str) -> i32;
    fn send_to_background(&mut self, child: u32, state: ProcessState, command: String);
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Defines whether the background process is running or stopped.
pub enum ProcessState {
    Running,
    Stopped,
    Empty,
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Stopped => write!(f, "Stopped"),
            ProcessState::Empty => write!(f, "Empty"),
        }
    }
}

pub(crate) fn add_to_background(
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    state: ProcessState,
    command: String,
) -> u32 {
    let mut processes = processes.lock().unwrap();
    match (*processes).iter().position(|x| x.state == ProcessState::Empty) {
        Some(id) => {
            (*processes)[id] =
                BackgroundProcess { pid, ignore_sighup: false, state, name: command };
            id as u32
        }
        None => {
            let njobs = (*processes).len();
            (*processes).push(BackgroundProcess {
                pid,
                ignore_sighup: false,
                state,
                name: command,
            });
            njobs as u32
        }
    }
}

#[derive(Clone, Debug)]
/// A background process is a process that is attached to, but not directly managed
/// by the shell. The shell will only retain information about the process, such
/// as the process ID, state that the process is in, and the command that the
/// process is executing.
pub struct BackgroundProcess {
    pub pid:           u32,
    pub ignore_sighup: bool,
    pub state:         ProcessState,
    pub name:          String,
}

impl JobControl for Shell {
    /// If a SIGTERM is received, a SIGTERM will be sent to all background processes
    /// before the shell terminates itself.
    fn handle_signal(&self, signal: i32) -> bool {
        if signal == sys::SIGTERM || signal == sys::SIGHUP {
            self.background_send(signal);
            true
        } else {
            false
        }
    }

    fn send_to_background(&mut self, pid: u32, state: ProcessState, command: String) {
        // Increment the `Arc` counters so that these fields can be moved into
        // the upcoming background thread.
        let processes = self.background.clone();
        let fg_signals = self.foreground_signals.clone();

        // Add the process to the background list, and mark the job's ID as
        // the previous job in the shell (in case fg/bg is executed w/ no args).
        let njob = add_to_background(processes.clone(), pid, state, command);
        self.previous_job = njob;
        eprintln!("ion: bg [{}] {}", njob, pid);

        // Spawn a background thread that will monitor the progress of the
        // background process, updating it's state changes until it finally
        // exits.
        let _ = spawn(move || {
            watch_background(fg_signals, processes, pid, njob as usize);
        });
    }

    /// Send a kill signal to all running background tasks.
    fn background_send(&self, signal: i32) {
        if signal == sys::SIGHUP {
            for process in self.background.lock().unwrap().iter() {
                if !process.ignore_sighup {
                    let _ = sys::killpg(process.pid, signal);
                }
            }
        } else {
            for process in self.background.lock().unwrap().iter() {
                if let ProcessState::Running = process.state {
                    let _ = sys::killpg(process.pid, signal);
                }
            }
        }
    }

    /// Resumes all stopped background jobs
    fn resume_stopped(&mut self) {
        for process in self.background.lock().unwrap().iter() {
            if process.state == ProcessState::Stopped {
                signals::resume(process.pid);
            }
        }
    }

    fn watch_foreground(&mut self, pid: i32, command: &str) -> i32 {
        let mut signaled = 0;
        let mut exit_status = 0;
        let mut status;

        loop {
            status = 0;
            match waitpid(pid, &mut status, WUNTRACED) {
                Err(errno) => match errno {
                    ECHILD if signaled == 0 => break exit_status,
                    ECHILD => break signaled,
                    errno => {
                        eprintln!("ion: waitpid error: {}", strerror(errno));
                        break FAILURE;
                    }
                },
                Ok(0) => (),
                Ok(_) if wifexited(status) => exit_status = wexitstatus(status),
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
                            let _ = kill(pid as u32, signal as i32);
                            self.break_flow = true;
                        }
                        _ => {
                            self.handle_signal(signal);
                        }
                    }
                    signaled = 128 + signal as i32;
                }
                Ok(pid) if wifstopped(status) => {
                    self.send_to_background(
                        pid.abs() as u32,
                        ProcessState::Stopped,
                        command.into(),
                    );
                    self.break_flow = true;
                    break 128 + wstopsig(status);
                }
                Ok(_) => (),
            }
        }
    }

    /// Waits until all running background tasks have completed, and listens for signals in the
    /// event that a signal is sent to kill the running tasks.
    fn wait_for_background(&mut self) {
        let sigcode;
        'event: loop {
            for process in self.background.lock().unwrap().iter() {
                if let ProcessState::Running = process.state {
                    while let Some(signal) = self.next_signal() {
                        if signal != sys::SIGTSTP {
                            self.background_send(signal);
                            sigcode = get_signal_code(signal);
                            break 'event;
                        }
                    }
                    sleep(Duration::from_millis(100));
                    continue 'event;
                }
            }
            return;
        }
        self.exit(sigcode);
    }

    fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> i32 {
        // Pass the TTY to the background job
        set_foreground_as(pid);
        // Signal the background thread that is waiting on this process to stop waiting.
        self.foreground_signals.signal_to_grab(pid);
        // Resume the background task, if needed.
        if cont {
            signals::resume(pid);
        }

        let status = loop {
            // When the background thread that is monitoring the task receives an exit/stop
            // signal, the status of that process will be communicated back. To
            // avoid consuming CPU cycles, we wait 25 ms between polls.
            match self.foreground_signals.was_processed() {
                Some(BackgroundResult::Status(stat)) => break i32::from(stat),
                Some(BackgroundResult::Errored) => break TERMINATED,
                None => sleep(Duration::from_millis(25)),
            }
        };
        // Have the shell reclaim the TTY
        set_foreground_as(process::id());
        status
    }
}

use crate::sys::{
    kill, strerror, waitpid, wcoredump, wexitstatus, wifcontinued, wifexited, wifsignaled,
    wifstopped, wstopsig, wtermsig, ECHILD, SIGINT, SIGPIPE, WCONTINUED, WNOHANG, WUNTRACED,
};

const OPTS: i32 = WUNTRACED | WCONTINUED | WNOHANG;

pub(crate) fn watch_background(
    fg: Arc<ForegroundSignals>,
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pgid: u32,
    njob: usize,
) {
    let (mut fg_was_grabbed, mut status);
    let mut exit_status = 0;

    macro_rules! get_process {
        (| $ident:ident | $func:expr) => {
            let mut processes = processes.lock().unwrap();
            let $ident = &mut processes.iter_mut().nth(njob).unwrap();
            $func
        };
    }

    loop {
        fg_was_grabbed = fg.was_grabbed(pgid);
        status = 0;
        match waitpid(-(pgid as i32), &mut status, OPTS) {
            Err(errno) if errno == ECHILD => {
                if !fg_was_grabbed {
                    eprintln!("ion: ([{}] {}) exited with {}", njob, pgid, status);
                }

                get_process!(|process| {
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.reply_with(exit_status as i8);
                    }
                });

                break;
            }
            Err(errno) => {
                eprintln!("ion: ([{}] {}) errored: {}", njob, pgid, errno);

                get_process!(|process| {
                    process.state = ProcessState::Empty;
                    if fg_was_grabbed {
                        fg.errored();
                    }
                });

                break;
            }
            Ok(0) => (),
            Ok(_) if wifexited(status) => exit_status = wexitstatus(status),
            Ok(_) if wifstopped(status) => {
                if !fg_was_grabbed {
                    eprintln!("ion: ([{}] {}) Stopped", njob, pgid);
                }

                get_process!(|process| {
                    if fg_was_grabbed {
                        fg.reply_with(TERMINATED as i8);
                    }
                    process.state = ProcessState::Stopped;
                });
            }
            Ok(_) if wifcontinued(status) => {
                if !fg_was_grabbed {
                    eprintln!("ion: ([{}] {}) Running", njob, pgid);
                }

                get_process!(|process| process.state = ProcessState::Running);
            }
            Ok(_) => (),
        }
        sleep(Duration::from_millis(100));
    }
}
