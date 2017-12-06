use super::foreground::BackgroundResult;
use super::super::Shell;
use super::super::signals;
use super::super::status::*;
use std::fmt;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::Duration;
use sys;

use sys::job_control as self_sys;
pub(crate) use sys::job_control::watch_background;

/// When given a process ID, that process's group will be assigned as the
/// foreground process group.
pub(crate) fn set_foreground_as(pid: u32) {
    signals::block();
    let _ = sys::tcsetpgrp(0, pid);
    signals::unblock();
}

pub(crate) trait JobControl {
    /// Waits for background jobs to finish before returning.
    fn wait_for_background(&mut self);
    /// Takes a background tasks's PID and whether or not it needs to be continued; resumes the
    /// task
    /// and sets it as the foreground process. Once the task exits or stops, the exit status
    /// will
    /// be returned, and ownership of the TTY given back to the shell.
    fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> i32;
    fn handle_signal(&self, signal: i32) -> bool;
    fn foreground_send(&self, signal: i32);
    fn background_send(&self, signal: i32);
    fn watch_foreground<F, D>(
        &mut self,
        pid: u32,
        last_pid: u32,
        get_command: F,
        drop_command: D,
    ) -> i32
    where
        F: FnOnce() -> String,
        D: FnMut(i32);
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
    match (*processes)
        .iter()
        .position(|x| x.state == ProcessState::Empty)
    {
        Some(id) => {
            (*processes)[id] = BackgroundProcess {
                pid: pid,
                ignore_sighup: false,
                state: state,
                name: command,
            };
            id as u32
        }
        None => {
            let njobs = (*processes).len();
            (*processes).push(BackgroundProcess {
                pid: pid,
                ignore_sighup: false,
                state: state,
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
    pub pid: u32,
    pub ignore_sighup: bool,
    pub state: ProcessState,
    pub name: String,
}

impl JobControl for Shell {
    fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> i32 {
        // Resume the background task, if needed.
        if cont {
            signals::resume(pid);
        }
        // Pass the TTY to the background job
        set_foreground_as(pid);
        // Signal the background thread that is waiting on this process to stop waiting.
        self.foreground_signals.signal_to_grab(pid);
        let status = loop {
            // When the background thread that is monitoring the task receives an exit/stop
            // signal, the status of that process will be communicated back. To
            // avoid consuming CPU cycles, we wait 25 ms between polls.
            match self.foreground_signals.was_processed() {
                Some(BackgroundResult::Status(stat)) => break stat as i32,
                Some(BackgroundResult::Errored) => break TERMINATED,
                None => sleep(Duration::from_millis(25)),
            }
        };
        // Have the shell reclaim the TTY
        set_foreground_as(process::id());
        status
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

    fn watch_foreground<F, D>(
        &mut self,
        pid: u32,
        last_pid: u32,
        get_command: F,
        drop_command: D,
    ) -> i32
    where
        F: FnOnce() -> String,
        D: FnMut(i32),
    {
        self_sys::watch_foreground(self, pid, last_pid, get_command, drop_command)
    }

    /// Send a kill signal to all running foreground tasks.
    fn foreground_send(&self, signal: i32) {
        for &process in self.foreground.iter() {
            let _ = sys::killpg(process, signal);
        }
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
}
