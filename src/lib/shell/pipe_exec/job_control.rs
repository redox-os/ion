use super::{
    foreground::{BackgroundResult, Signals},
    PipelineError,
};
use crate::{
    builtins::Status,
    shell::{signals, BackgroundEventCallback, Shell},
};
use nix::{
    sys::{
        signal::{self, Signal},
        wait::{self, WaitPidFlag, WaitStatus},
    },
    unistd::{self, Pid},
};
use std::{
    fmt,
    sync::Mutex,
    thread::{sleep, spawn},
    time::Duration,
};

#[derive(Clone, Copy, Hash, Debug, PartialEq)]
/// Defines whether the background process is running or stopped.
pub enum ProcessState {
    Running,
    Stopped,
    Empty,
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Stopped => write!(f, "Stopped"),
            ProcessState::Empty => write!(f, "Empty"),
        }
    }
}

/// An event sent by a job watcher for a background job
#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundEvent {
    /// A new job was sent to background
    Added,
    /// A background job was stopped
    Stopped,
    /// A background job was resumed
    Resumed,
    /// A background job exited
    Exited(i32),
    /// A job errored
    Errored(nix::Error),
}

#[derive(Clone, Debug, Hash)]
/// A background process is a process that is attached to, but not directly managed
/// by the shell. The shell will only retain information about the process, such
/// as the process ID, state that the process is in, and the command that the
/// process is executing. Note that it is necessary to check if the process exists with the exists
/// method.
pub struct BackgroundProcess {
    pid:           Pid,
    ignore_sighup: bool,
    state:         ProcessState,
    name:          String,
}

impl BackgroundProcess {
    pub(super) const fn new(pid: Pid, state: ProcessState, name: String) -> Self {
        Self { pid, ignore_sighup: false, state, name }
    }

    /// Get the pid associated with the job
    pub const fn pid(&self) -> Pid { self.pid }

    /// Check if the process is still running
    pub fn is_running(&self) -> bool { self.state == ProcessState::Running }

    /// Check if this is in fact a process
    pub fn exists(&self) -> bool { self.state != ProcessState::Empty }

    /// Stop capturing information about the process. *This action is irreversible*
    pub fn forget(&mut self) { self.state = ProcessState::Empty }

    /// Should the process ignore sighups
    pub fn set_ignore_sighup(&mut self, ignore: bool) { self.ignore_sighup = ignore }

    /// resume a stopped job
    pub fn resume(&self) { signals::resume(self.pid); }
}

impl fmt::Display for BackgroundProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}\t{}", self.pid, self.state, self.name)
    }
}

impl<'a> Shell<'a> {
    /// If a SIGTERM is received, a SIGTERM will be sent to all background processes
    /// before the shell terminates itself.
    pub fn handle_signal(&self, signal: Signal) -> nix::Result<bool> {
        if signal == Signal::SIGTERM || signal == Signal::SIGHUP {
            self.background_send(signal)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn add_to_background(&mut self, job: BackgroundProcess) -> usize {
        let mut processes = self.background_jobs_mut();
        if let Some(id) = processes.iter().position(|x| !x.exists()) {
            processes[id] = job;
            id
        } else {
            let njobs = processes.len();
            processes.push(job);
            njobs
        }
    }

    fn watch_background(
        fg: &Signals,
        processes: &Mutex<Vec<BackgroundProcess>>,
        pgid: Pid,
        njob: usize,
        background_event: &Option<BackgroundEventCallback>,
    ) {
        let mut exit_status = 0;

        macro_rules! get_process {
            (| $ident:ident | $func:expr) => {
                let mut processes = processes.lock().unwrap();
                let $ident = processes.get_mut(njob).unwrap();
                $func
            };
        }

        loop {
            let fg_was_grabbed = fg.was_grabbed(pgid);
            let mut opts = WaitPidFlag::WUNTRACED;
            opts.insert(WaitPidFlag::WCONTINUED);
            opts.insert(WaitPidFlag::WNOHANG);
            match wait::waitpid(Pid::from_raw(-pgid.as_raw()), Some(opts)) {
                Err(nix::Error::Sys(nix::errno::Errno::ECHILD)) => {
                    if !fg_was_grabbed {
                        if let Some(ref callback) = &background_event {
                            callback(njob, pgid, BackgroundEvent::Exited(exit_status));
                        }
                    }

                    get_process!(|process| {
                        process.forget();
                        if fg_was_grabbed {
                            fg.reply_with(exit_status);
                        }
                    });

                    break;
                }
                Err(errno) => {
                    if let Some(ref callback) = &background_event {
                        callback(njob, pgid, BackgroundEvent::Errored(errno));
                    }

                    get_process!(|process| {
                        process.forget();
                        if fg_was_grabbed {
                            fg.errored();
                        }
                    });

                    break;
                }
                Ok(WaitStatus::Exited(_, status)) => exit_status = status,
                Ok(WaitStatus::Stopped(..)) => {
                    if !fg_was_grabbed {
                        if let Some(ref callback) = &background_event {
                            callback(njob, pgid, BackgroundEvent::Stopped);
                        }
                    }

                    get_process!(|process| {
                        if fg_was_grabbed {
                            fg.reply_with(Status::TERMINATED.as_os_code());
                        }
                        process.state = ProcessState::Stopped;
                    });
                }
                Ok(WaitStatus::Continued(_)) => {
                    if !fg_was_grabbed {
                        if let Some(ref callback) = &background_event {
                            callback(njob, pgid, BackgroundEvent::Resumed);
                        }
                    }

                    get_process!(|process| process.state = ProcessState::Running);
                }
                Ok(_) => (),
            }
            sleep(Duration::from_millis(100));
        }
    }

    /// Send the current job to the background and spawn a thread to update its state
    pub fn send_to_background(&mut self, process: BackgroundProcess) {
        // Add the process to the background list, and mark the job's ID as
        // the previous job in the shell (in case fg/bg is executed w/ no args).
        let pid = process.pid();
        let njob = self.add_to_background(process);
        self.previous_job = njob;
        if let Some(ref callback) = &self.background_event {
            callback(njob, pid, BackgroundEvent::Added);
        }

        // Increment the `Arc` counters so that these fields can be moved into
        // the upcoming background thread.
        let processes = self.background.clone();
        let fg_signals = self.foreground_signals.clone();
        let background_event = self.background_event.clone();
        // Spawn a background thread that will monitor the progress of the
        // background process, updating it's state changes until it finally
        // exits.
        let _ = spawn(move || {
            Self::watch_background(&fg_signals, &processes, pid, njob as usize, &background_event)
        });
    }

    /// Send a kill signal to all running background tasks.
    pub fn background_send(&self, signal: Signal) -> nix::Result<()> {
        let filter: fn(&&BackgroundProcess) -> bool =
            if signal == Signal::SIGHUP { |p| !p.ignore_sighup } else { |p| p.is_running() };
        self.background_jobs()
            .iter()
            .filter(filter)
            .map(|p| signal::killpg(p.pid(), signal))
            .find(Result::is_err)
            .unwrap_or_else(|| Ok(()))
    }

    /// Resumes all stopped background jobs
    pub fn resume_stopped(&mut self) {
        for process in self.background_jobs().iter().filter(|p| p.state == ProcessState::Stopped) {
            signals::resume(process.pid());
        }
    }

    /// Wait for the job in foreground
    pub fn watch_foreground(&mut self, group: Pid) -> Result<Status, PipelineError> {
        let mut signaled = None;
        let mut exit_status = Status::SUCCESS;

        loop {
            match wait::waitpid(Pid::from_raw(-group.as_raw()), Some(WaitPidFlag::WUNTRACED)) {
                Err(err) => match err {
                    nix::Error::Sys(nix::errno::Errno::ECHILD) => {
                        if let Some(signal) = signaled {
                            break Err(signal);
                        } else {
                            break Ok(exit_status);
                        }
                    }
                    err => break Err(PipelineError::WaitPid(err)),
                },
                Ok(WaitStatus::Exited(_, status)) => exit_status = Status::from_exit_code(status),
                Ok(WaitStatus::Signaled(pid, signal, core_dumped)) => {
                    if signal == signal::Signal::SIGPIPE {
                    } else if core_dumped {
                        signaled = Some(PipelineError::CoreDump(pid));
                    } else {
                        if signal == Signal::SIGINT {
                            let _ = signal::kill(pid, signal);
                        } else {
                            let _ = self.handle_signal(signal);
                        }
                        signaled = Some(PipelineError::Interrupted(pid, signal));
                    }
                }
                Ok(WaitStatus::Stopped(pid, signal)) => {
                    self.send_to_background(BackgroundProcess::new(
                        pid,
                        ProcessState::Stopped,
                        "".to_string(),
                    ));
                    break Err(PipelineError::Interrupted(pid, signal));
                }
                Ok(_) => (),
            }
        }
    }

    /// Waits until all running background tasks have completed, and listens for signals in the
    /// event that a signal is sent to kill the running tasks.
    pub fn wait_for_background(&mut self) -> Result<(), PipelineError> {
        while { self.background_jobs().iter().any(BackgroundProcess::is_running) } {
            if let Some(signal) = signals::SignalHandler.find(|&s| s != Signal::SIGTSTP) {
                self.background_send(signal).map_err(PipelineError::KillFailed)?;
                return Err(PipelineError::Interrupted(Pid::this(), signal));
            }
            sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// When given a process ID, that process's group will be assigned as the
    /// foreground process group.
    fn set_foreground_as(pid: Pid) {
        signals::block();
        unistd::tcsetpgrp(0, pid).unwrap();
        signals::unblock();
    }

    /// Takes a background tasks's PID and whether or not it needs to be continued; resumes the
    /// task and sets it as the foreground process. Once the task exits or stops, the exit status
    /// will be returned, and ownership of the TTY given back to the shell.
    pub fn set_bg_task_in_foreground(&self, pid: Pid, cont: bool) -> Status {
        // Pass the TTY to the background job
        Self::set_foreground_as(pid);
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
                Some(BackgroundResult::Status(stat)) => break Status::from_exit_code(stat),
                Some(BackgroundResult::Errored) => break Status::TERMINATED,
                None => sleep(Duration::from_millis(25)),
            }
        };
        // Have the shell reclaim the TTY
        Self::set_foreground_as(Pid::this());
        status
    }
}
