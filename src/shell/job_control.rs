#[cfg(all(unix, not(target_os = "redox")))] use libc::{self, pid_t, c_int};
#[cfg(all(unix, not(target_os = "redox")))] use nix::sys::signal::{self, Signal as NixSignal};
use std::fmt;
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::process;
use std::sync::{Arc, Mutex};
use super::status::*;
use super::Shell;

pub trait JobControl {
    fn wait_for_background(&mut self);
    fn handle_signal(&self, signal: i32);
    fn foreground_send(&self, signal: i32);
    fn background_send(&self, signal: i32);
    fn watch_foreground(&mut self, pid: u32) -> i32;
    fn send_to_background(&mut self, child: u32, state: ProcessState);
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

#[cfg(target_os = "redox")]
pub fn watch_background_pid(processes: Arc<Mutex<Vec<BackgroundProcess>>>, pid: u32, njob: usize) {
    // TODO: Implement this using syscall::call::waitpid
}

#[cfg(all(unix, not(target_os = "redox")))]
pub fn watch_background_pid (
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    njob: usize)
{
    use nix::sys::wait::{waitpid, WaitStatus, WUNTRACED, WNOHANG};
    loop {
        match waitpid(-(pid as pid_t), Some(WUNTRACED | WNOHANG)) {
            Ok(WaitStatus::Exited(_, status)) => {
                eprintln!("ion: ([{}] {}) exited with {}", njob, pid, status);
                let mut processes = processes.lock().unwrap();
                let process = &mut processes.iter_mut().nth(njob).unwrap();
                process.state = ProcessState::Empty;
                break
            },
            Ok(WaitStatus::Stopped(pid, _)) => {
                eprintln!("ion: ([{}] {}) Stopped", njob, pid);
                let mut processes = processes.lock().unwrap();
                let process = &mut processes.iter_mut().nth(njob).unwrap();
                process.state = ProcessState::Stopped;
            },
            Ok(WaitStatus::Continued(pid)) => {
                eprintln!("ion: ([{}] {}) Running", njob, pid);
                let mut processes = processes.lock().unwrap();
                let process = &mut processes.iter_mut().nth(njob).unwrap();
                process.state = ProcessState::Running;
            },
            Ok(_) => (),
            Err(why) => {
                eprintln!("ion: ([{}] {}) errored: {}", njob, pid, why);
                let mut processes = processes.lock().unwrap();
                let process = &mut processes.iter_mut().nth(njob).unwrap();
                process.state = ProcessState::Empty;
                break
            }
        }
        sleep(Duration::from_millis(100));
    }
}

pub fn add_to_background (
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    state: ProcessState
) -> usize {
    let mut processes = processes.lock().unwrap();
    match (*processes).iter().position(|x| {
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
    #[cfg(all(unix, not(target_os = "redox")))]
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

    #[cfg(all(unix, not(target_os = "redox")))]
    fn watch_foreground(&mut self, pid: u32) -> i32 {
        use nix::sys::wait::{waitpid, WaitStatus, WUNTRACED, WNOHANG};
        use nix::sys::signal::Signal;
        loop {
            match waitpid(-(pid as pid_t), Some(WUNTRACED | WNOHANG)) {
                Ok(WaitStatus::Exited(_, status)) => {
                    break status as i32
                },
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    eprintln!("ion: process ended by signal");
                    if signal == Signal::SIGTERM {
                        self.handle_signal(libc::SIGTERM);
                    } else if signal == Signal::SIGINT {
                        self.foreground_send(libc::SIGINT as i32);
                    }
                    break TERMINATED;
                },
                Ok(WaitStatus::Stopped(pid, _)) => {
                    self.send_to_background(pid as u32, ProcessState::Stopped);
                    self.received_sigtstp = true;
                    break TERMINATED
                },
                Ok(_) => (),
                Err(why) => {
                    eprintln!("ion: process doesn't exist: {}", why);
                    break FAILURE
                }
            }
            sleep(Duration::from_millis(1));
        }
    }

    #[cfg(target_os = "redox")]
    fn watch_foreground(&mut self, pid: u32) -> i32 {
        use std::io::{self, Write};
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;
        use syscall;
        use syscall::flag::WNOHANG;

        loop {
            let mut status_raw = 0;
            match syscall::waitpid(pid as usize, &mut status_raw, WNOHANG) {
                Ok(0) => (),
                Ok(_pid) => {
                    let status = ExitStatus::from_raw(status_raw as i32);
                    if let Some(code) = status.code() {
                        break code
                    } else {
                        let stderr = io::stderr();
                        let mut stderr = stderr.lock();
                        let _ = stderr.write_all(b"ion: child ended by signal\n");
                        break TERMINATED
                    }
                },
                Err(err) => {
                    let stderr = io::stderr();
                    let mut stderr = stderr.lock();
                    let _ = writeln!(stderr, "ion: failed to wait: {}", err);
                    break 100 // TODO what should we return here?
                }
            }
            sleep(Duration::from_millis(1));
        }
    }

    #[cfg(all(unix, not(target_os = "redox")))]
    /// Send a kill signal to all running foreground tasks.
    fn foreground_send(&self, signal: i32) {
        for process in self.foreground.iter() {
            let _ = signal::kill(-(*process as pid_t), NixSignal::from_c_int(signal as c_int).ok());
        }
    }

    #[cfg(target_os = "redox")]
    fn foreground_send(&self, _: i32) {
        // TODO: Redox doesn't support signals yet
    }

    #[cfg(all(unix, not(target_os = "redox")))]
    /// Send a kill signal to all running background tasks.
    fn background_send(&self, signal: i32) {
        for process in self.background.lock().unwrap().iter() {
            if let ProcessState::Running = process.state {
                let _ = signal::kill(-(process.pid as pid_t), NixSignal::from_c_int(signal as c_int).ok());
            }
        }
    }

    #[cfg(target_os = "redox")]
    fn background_send(&self, _: i32) {
        // TODO: Redox doesn't support signals yet
    }

    fn send_to_background(&mut self, pid: u32, state: ProcessState) {
        let processes = self.background.clone();
        let _ = spawn(move || {
            let njob = add_to_background(processes.clone(), pid, state);
            eprintln!("ion: bg [{}] {}", njob, pid);
            watch_background_pid(processes, pid, njob);
        });
    }

    /// If a SIGTERM is received, a SIGTERM will be sent to all background processes
    /// before the shell terminates itself.
    #[cfg(all(unix, not(target_os = "redox")))]
    fn handle_signal(&self, signal: i32) {
        if signal == libc::SIGTERM {
            self.background_send(libc::SIGTERM);
            process::exit(TERMINATED);
        }
    }

    #[cfg(target_os = "redox")]
    fn handle_signal(&self, _: i32) {
        // TODO: Redox doesn't support signals yet;
    }
}
