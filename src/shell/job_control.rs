#[cfg(all(unix, not(target_os = "redox")))] use libc::{self, pid_t, c_int};
#[cfg(all(unix, not(target_os = "redox")))] use nix::sys::signal::{self, Signal};
#[cfg(all(unix, not(target_os = "redox")))] use nix::unistd;
use std::fmt;
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use super::foreground::{ForegroundSignals, BackgroundResult};
use super::signals;
use super::status::*;
use super::Shell;
use super::pipe::crossplat::get_pid;

#[cfg(all(unix, not(target_os = "redox")))]
/// When given a process ID, that process's group will be assigned as the foreground process group.
pub fn set_foreground_as(pid: u32) {
    signals::block();
    let _ = unistd::tcsetpgrp(0, pid as i32);
    signals::unblock();
}

#[cfg(target_os = "redox")]
pub fn set_foreground_as(pid: u32) {
    // TODO
}

pub trait JobControl {
    /// Waits for background jobs to finish before returning.
    fn wait_for_background(&mut self);
    /// Takes a background tasks's PID and whether or not it needs to be continued; resumes the task
    /// and sets it as the foreground process. Once the task exits or stops, the exit status will
    /// be returned, and ownership of the TTY given back to the shell.
    fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> i32;
    fn handle_signal(&self, signal: i32) -> bool;
    fn foreground_send(&self, signal: i32);
    fn background_send(&self, signal: i32);
    fn watch_foreground <F, D> (
        &mut self,
        pid: u32,
        last_pid: u32,
        get_command: F,
        drop_command: D
        ) -> i32 where F: FnOnce() -> String,
                       D: FnMut(i32);
    fn send_to_background(&mut self, child: u32, state: ProcessState, command: String);
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
pub fn watch_background (
    fg: Arc<ForegroundSignals>,
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    njob: usize
) {
    // TODO: Implement this using syscall::call::waitpid
}

#[cfg(all(unix, not(target_os = "redox")))]
pub fn watch_background (
    fg: Arc<ForegroundSignals>,
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    njob: usize
) {
    use nix::sys::wait::*;
    let mut fg_was_grabbed = false;
    loop {
        if !fg_was_grabbed {
            if fg.was_grabbed(pid) { fg_was_grabbed = true; }
        }
        match waitpid(-(pid as pid_t), Some(WUNTRACED | WCONTINUED | WNOHANG)) {
            Ok(WaitStatus::Exited(_, status)) => {
                if !fg_was_grabbed {
                    eprintln!("ion: ([{}] {}) exited with {}", njob, pid, status);
                }
                let mut processes = processes.lock().unwrap();
                let process = &mut processes.iter_mut().nth(njob).unwrap();
                process.state = ProcessState::Empty;
                if fg_was_grabbed { fg.reply_with(status); }
                break
            },
            Ok(WaitStatus::Stopped(pid, _)) => {
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
            },
            Ok(WaitStatus::Continued(pid)) => {
                if !fg_was_grabbed {
                    eprintln!("ion: ([{}] {}) Running", njob, pid);
                }
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
                if fg_was_grabbed { fg.errored(); }
                break
            }
        }
        sleep(Duration::from_millis(100));
    }
}

pub fn add_to_background (
    processes: Arc<Mutex<Vec<BackgroundProcess>>>,
    pid: u32,
    state: ProcessState,
    command: String
) -> usize {
    let mut processes = processes.lock().unwrap();
    match (*processes).iter().position(|x| x.state == ProcessState::Empty) {
        Some(id) => {
            (*processes)[id] = BackgroundProcess {
                pid: pid,
                ignore_sighup: false,
                state: state,
                name: command
            };
            id
        },
        None => {
            let njobs = (*processes).len();
            (*processes).push(BackgroundProcess {
                pid: pid,
                ignore_sighup: false,
                state: state,
                name: command
            });
            njobs
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
    pub name: String
}

impl<'a> JobControl for Shell<'a> {
    fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> i32 {
        // Resume the background task, if needed.
        if cont { signals::resume(pid); }
        // Pass the TTY to the background job
        set_foreground_as(pid);
        // Signal the background thread that is waiting on this process to stop waiting.
        self.foreground_signals.signal_to_grab(pid);
        let status = loop {
            // When the background thread that is monitoring the task receives an exit/stop signal,
            // the status of that process will be communicated back. To avoid consuming CPU cycles,
            // we wait 25 ms between polls.
            match self.foreground_signals.was_processed() {
                Some(BackgroundResult::Status(stat)) => break stat as i32,
                Some(BackgroundResult::Errored) => break TERMINATED,
                None => sleep(Duration::from_millis(25))
            }
        };
        // Have the shell reclaim the TTY
        set_foreground_as(get_pid());
        status
    }

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
                            break 'event
                        }
                    }
                    sleep(Duration::from_millis(100));
                    continue 'event
                }
            }
            return
        }
        self.exit(TERMINATED);
    }

    #[cfg(target_os = "redox")]
    fn wait_for_background(&mut self) {
        // TODO: Redox doesn't support signals yet.
    }

    #[cfg(all(unix, not(target_os = "redox")))]
    fn watch_foreground <F: FnOnce() -> String, D: FnMut(i32)> (
        &mut self,
        pid: u32,
        last_pid: u32,
        get_command: F,
        mut drop_command: D,
    ) -> i32 {
        use nix::sys::wait::{waitpid, WaitStatus, WUNTRACED};
        use nix::sys::signal::Signal;
        use nix::{Error, Errno};
        let mut exit_status = 0;
        loop {
            match waitpid(-(pid as pid_t), Some(WUNTRACED)) {
                Ok(WaitStatus::Exited(pid, status)) => {
                    if pid == (last_pid as i32) {
                        break status as i32
                    } else {
                        drop_command(pid);
                        exit_status = status;
                    }
                }
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    eprintln!("ion: process ended by signal");
                    if signal == Signal::SIGTERM {
                        self.handle_signal(libc::SIGTERM);
                        self.exit(TERMINATED);
                    } else if signal == Signal::SIGHUP {
                        self.handle_signal(libc::SIGHUP);
                        self.exit(TERMINATED);
                    } else if signal == Signal::SIGINT {
                        self.foreground_send(libc::SIGINT as i32);
                        self.break_flow = true;
                    }
                    break TERMINATED;
                },
                Ok(WaitStatus::Stopped(pid, _)) => {
                    self.send_to_background(pid as u32, ProcessState::Stopped, get_command());
                    self.break_flow = true;
                    break TERMINATED
                },
                Ok(_) => (),
                // ECHILD signifies that all children have exited
                Err(Error::Sys(Errno::ECHILD)) => break exit_status as i32,
                Err(why) => {
                    eprintln!("ion: process doesn't exist: {}", why);
                    break FAILURE
                }
            }
        }
    }

    #[cfg(target_os = "redox")]
    fn watch_foreground <F: FnOnce() -> String, D: FnMut(i32)> (
        &mut self,
        pid: u32,
        _last_pid: u32,
        _get_command: F,
        mut drop_command: D,
    ) -> i32 {
        use std::io::{self, Write};
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;
        use syscall;
        use syscall::flag::WNOHANG;

        loop {
            let mut status_raw = 0;
            match syscall::waitpid(pid as usize, &mut status_raw, 0) {
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
        }
    }

    #[cfg(all(unix, not(target_os = "redox")))]
    /// Send a kill signal to all running foreground tasks.
    fn foreground_send(&self, signal: i32) {
        for process in self.foreground.iter() {
            let _ = signal::kill(-(*process as pid_t), Signal::from_c_int(signal as c_int).ok());
        }
    }

    #[cfg(target_os = "redox")]
    fn foreground_send(&self, _: i32) {
        // TODO: Redox doesn't support signals yet
    }

    #[cfg(all(unix, not(target_os = "redox")))]
    /// Send a kill signal to all running background tasks.
    fn background_send(&self, signal: i32) {
        if signal == libc::SIGHUP {
            for process in self.background.lock().unwrap().iter() {
                if !process.ignore_sighup {
                    let _ = signal::kill(-(process.pid as pid_t), Signal::from_c_int(signal as c_int).ok());
                }
            }
        } else {
            for process in self.background.lock().unwrap().iter() {
                if let ProcessState::Running = process.state {
                    let _ = signal::kill(-(process.pid as pid_t), Signal::from_c_int(signal as c_int).ok());
                }
            }
        }
    }

    #[cfg(target_os = "redox")]
    fn background_send(&self, _: i32) {
        // TODO: Redox doesn't support signals yet
    }

    fn send_to_background(&mut self, pid: u32, state: ProcessState, command: String) {
        let processes = self.background.clone();
        let fg_signals = self.foreground_signals.clone();
        let _ = spawn(move || {
            let njob = add_to_background(processes.clone(), pid, state, command);
            eprintln!("ion: bg [{}] {}", njob, pid);
            watch_background(fg_signals, processes, pid, njob);
        });
    }

    /// If a SIGTERM is received, a SIGTERM will be sent to all background processes
    /// before the shell terminates itself.
    #[cfg(all(unix, not(target_os = "redox")))]
    fn handle_signal(&self, signal: i32) -> bool {
        if signal == libc::SIGTERM || signal == libc::SIGHUP {
            self.background_send(signal);
            true
        } else { false }
    }

    #[cfg(target_os = "redox")]
    fn handle_signal(&self, _: i32) -> bool {
        // TODO: Redox doesn't support signals yet;
        false
    }
}
