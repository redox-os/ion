#[cfg(not(target_os = "redox"))] use libc::{self, pid_t, c_int};
#[cfg(not(target_os = "redox"))] use nix::sys::signal::{self, Signal as NixSignal};
use std::fmt;
use std::io::{stderr, Write};
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::process::{self, Child};
use super::status::*;
use super::Shell;

pub trait JobControl {
    fn suspend(&mut self, pid: u32);
    fn wait_for_background(&mut self);
    fn handle_signal(&self, signal: i32);
    fn foreground_send(&self, signal: i32);
    fn background_send(&self, signal: i32);
    fn send_child_to_background(&mut self, child: Child, state: ProcessState);
}

#[derive(Clone)]
/// Defines whether the background process is running or stopped.
pub enum ProcessState {
    Running,
    Stopped,
    Empty
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Stopped => write!(f, "Stopped"),
            ProcessState::Empty   => write!(f, "Empty"),
        }
    }
}

#[derive(Clone)]
/// A background process is a process that is attached to, but not directly managed
/// by the shell. The shell will only retain information about the process, such
/// as the process ID, state that the process is in, and the command that the
/// process is executing.
pub struct BackgroundProcess {
    pub pid: u32,
    pub state: ProcessState
    // TODO: Each process should have the command registered to it
    // pub command: String
}

impl<'a> JobControl for Shell<'a> {
    #[cfg(not(target_os = "redox"))]
    /// Suspends a given process by it's process ID.
    fn suspend(&mut self, pid: u32) {
        let _ = signal::kill(pid as pid_t, Some(NixSignal::SIGTSTP));
    }

    #[cfg(target_os = "redox")]
    fn suspend(&mut self, _: u32) {
        // TODO: Redox doesn't support signals yet.
    }

    #[cfg(not(target_os = "redox"))]
    /// Waits until all running background tasks have completed, and listens for signals in the
    /// event that a signal is sent to kill the running tasks.
    fn wait_for_background(&mut self) {
        'event: loop {
            for process in self.background.lock().unwrap().iter() {
                if let ProcessState::Running = process.state {
                    if let Ok(signal) = self.signals.try_recv() {
                        if signal != libc::SIGTSTP {
                            self.background_send(signal);
                            process::exit(TERMINATED);
                        }
                    }
                    sleep(Duration::from_millis(100));
                    continue 'event
                }
            }
            break
        }
    }

    #[cfg(target_os = "redox")]
    fn wait_for_background(&mut self) {
        // TODO: Redox doesn't support signals yet.
    }

    #[cfg(not(target_os = "redox"))]
    /// Send a kill signal to all running foreground tasks.
    fn foreground_send(&self, signal: i32) {
        for process in self.foreground.iter() {
            let _ = signal::kill(*process as pid_t, NixSignal::from_c_int(signal as c_int).ok());
        }
    }

    #[cfg(target_os = "redox")]
    fn foreground_send(&self, _: i32) {
        // TODO: Redox doesn't support signals yet
    }

    #[cfg(not(target_os = "redox"))]
    /// Send a kill signal to all running background tasks.
    fn background_send(&self, signal: i32) {
        for process in self.background.lock().unwrap().iter() {
            if let ProcessState::Running = process.state {
                let _ = signal::kill(process.pid as pid_t, NixSignal::from_c_int(signal as c_int).ok());
            }
        }
    }

    #[cfg(target_os = "redox")]
    fn background_send(&self, _: i32) {
        // TODO: Redox doesn't support signals yet
    }

    fn send_child_to_background(&mut self, mut child: Child, state: ProcessState) {
        let pid = child.id();
        let processes = self.background.clone();
        let _ = spawn(move || {
            let njob;
            {
                let mut processes = processes.lock().unwrap();
                njob = match (*processes).iter().position(|x| {
                    if let ProcessState::Empty = x.state { true } else { false }
                }) {
                    Some(id) => {
                        (*processes)[id] = BackgroundProcess {
                            pid: pid,
                            state: state
                        };
                        id
                    },
                    None => {
                        let njobs = (*processes).len();
                        (*processes).push(BackgroundProcess {
                            pid: pid,
                            state: state
                        });
                        njobs
                    }
                };

                let stderr = stderr();
                let _ = writeln!(stderr.lock(), "ion: bg: [{}] {}", njob, pid);
            }

            // Wait for the child to complete before removing it from the process list.
            let status = child.wait();

            // Notify the user that the background task has completed.
            let stderr = stderr();
            let mut stderr = stderr.lock();
            let _ = match status {
                Ok(status) => writeln!(stderr, "ion: bg: [{}] {} completed: {}", njob, pid, status),
                Err(why)   => writeln!(stderr, "ion: bg: [{}] {} errored: {}", njob, pid, why)
            };

            // Remove the process from the background processes list.
            let mut processes = processes.lock().unwrap();
            let process = &mut processes.iter_mut().nth(njob).unwrap();
            process.state = ProcessState::Empty;
        });
    }

    /// If a SIGTERM is received, a SIGTERM will be sent to all background processes
    /// before the shell terminates itself.
    fn handle_signal(&self, signal: i32) {
        if signal == libc::SIGTERM {
            self.background_send(libc::SIGTERM);
            process::exit(TERMINATED);
        }
    }
}
