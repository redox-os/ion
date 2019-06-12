use super::{
    super::{signals, status::*, Shell},
    foreground::{BackgroundResult, ForegroundSignals},
    PipelineError,
};
use crate::sys::{
    self, kill, strerror, waitpid, wcoredump, wexitstatus, wifcontinued, wifexited, wifsignaled,
    wifstopped, wstopsig, wtermsig, ECHILD, SIGINT, SIGPIPE, WCONTINUED, WNOHANG, WUNTRACED,
};
use std::{
    fmt, process,
    sync::Mutex,
    thread::{sleep, spawn},
    time::Duration,
};

const OPTS: i32 = WUNTRACED | WCONTINUED | WNOHANG;

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

#[derive(Clone, Debug, Hash)]
/// A background process is a process that is attached to, but not directly managed
/// by the shell. The shell will only retain information about the process, such
/// as the process ID, state that the process is in, and the command that the
/// process is executing.
pub struct BackgroundProcess {
    pid:           u32,
    ignore_sighup: bool,
    state:         ProcessState,
    name:          String,
}

impl BackgroundProcess {
    pub(super) fn new(pid: u32, state: ProcessState, name: String) -> Self {
        BackgroundProcess { pid, ignore_sighup: false, state, name }
    }

    pub fn pid(&self) -> u32 { self.pid }

    pub fn is_running(&self) -> bool { self.state == ProcessState::Running }

    pub fn exists(&self) -> bool { self.state == ProcessState::Empty }

    pub fn forget(&mut self) { self.state = ProcessState::Empty }

    pub fn set_ignore_sighup(&mut self, ignore: bool) { self.ignore_sighup = ignore }

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
    pub fn handle_signal(&self, signal: i32) -> bool {
        if signal == sys::SIGTERM || signal == sys::SIGHUP {
            self.background_send(signal);
            true
        } else {
            false
        }
    }

    fn add_to_background(&mut self, job: BackgroundProcess) -> usize {
        let mut processes = self.background_jobs_mut();
        match processes.iter().position(|x| !x.exists()) {
            Some(id) => {
                processes[id] = job;
                id
            }
            None => {
                let njobs = processes.len();
                processes.push(job);
                njobs
            }
        }
    }

    fn watch_background(
        fg: &ForegroundSignals,
        processes: &Mutex<Vec<BackgroundProcess>>,
        pgid: u32,
        njob: usize,
    ) {
        let mut exit_status = 0;

        macro_rules! get_process {
            (| $ident:ident | $func:expr) => {
                let mut processes = processes.lock().unwrap();
                let $ident = &mut processes.get_mut(njob).unwrap();
                $func
            };
        }

        loop {
            let fg_was_grabbed = fg.was_grabbed(pgid);
            let mut status = 0;
            match waitpid(-(pgid as i32), &mut status, OPTS) {
                Err(errno) if errno == ECHILD => {
                    if !fg_was_grabbed {
                        eprintln!("ion: ([{}] {}) exited with {}", njob, pgid, status);
                    }

                    get_process!(|process| {
                        process.forget();
                        if fg_was_grabbed {
                            fg.reply_with(exit_status as i8);
                        }
                    });

                    break;
                }
                Err(errno) => {
                    eprintln!("ion: ([{}] {}) errored: {}", njob, pgid, errno);

                    get_process!(|process| {
                        process.forget();
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
                            fg.reply_with(Status::TERMINATED.as_os_code() as i8);
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

    pub fn send_to_background(&mut self, process: BackgroundProcess) {
        // Add the process to the background list, and mark the job's ID as
        // the previous job in the shell (in case fg/bg is executed w/ no args).
        let pid = process.pid();
        let njob = self.add_to_background(process);
        self.previous_job = njob;
        eprintln!("ion: bg [{}] {}", njob, pid);

        // Increment the `Arc` counters so that these fields can be moved into
        // the upcoming background thread.
        let processes = self.background.clone();
        let fg_signals = self.foreground_signals.clone();
        // Spawn a background thread that will monitor the progress of the
        // background process, updating it's state changes until it finally
        // exits.
        let _ = spawn(move || Self::watch_background(&fg_signals, &processes, pid, njob as usize));
    }

    /// Send a kill signal to all running background tasks.
    pub fn background_send(&self, signal: i32) {
        let filter: fn(&&BackgroundProcess) -> bool =
            if signal == sys::SIGHUP { |p| !p.ignore_sighup } else { |p| p.is_running() };
        self.background_jobs().iter().filter(filter).for_each(|p| {
            let _ = sys::killpg(p.pid(), signal);
        })
    }

    /// Resumes all stopped background jobs
    pub fn resume_stopped(&mut self) {
        for process in self.background_jobs().iter().filter(|p| p.state == ProcessState::Stopped) {
            signals::resume(process.pid());
        }
    }

    pub fn watch_foreground(&mut self, pgid: u32) -> Result<Status, PipelineError> {
        let mut signaled = None;
        let mut exit_status = Status::SUCCESS;

        loop {
            let mut status = 0;
            match waitpid(-(pgid as i32), &mut status, WUNTRACED) {
                Err(errno) => match errno {
                    ECHILD => {
                        if let Some(signal) = signaled {
                            break Err(signal);
                        } else {
                            break Ok(exit_status);
                        }
                    }
                    errno => break Err(PipelineError::WaitPid(strerror(errno))),
                },
                Ok(0) => (),
                Ok(_) if wifexited(status) => {
                    exit_status = Status::from_exit_code(wexitstatus(status))
                }
                Ok(pid) if wifsignaled(status) => {
                    let signal = wtermsig(status);
                    if signal == SIGPIPE {
                    } else if wcoredump(status) {
                        signaled = Some(PipelineError::CoreDump(pid as u32));
                    } else {
                        match signal {
                            SIGINT => {
                                let _ = kill(pid as u32, signal as i32);
                            }
                            _ => {
                                self.handle_signal(signal);
                            }
                        }
                        signaled = Some(PipelineError::Interrupted(pid as u32, signal));
                    }
                }
                Ok(pid) if wifstopped(status) => {
                    self.send_to_background(BackgroundProcess::new(
                        pid.abs() as u32,
                        ProcessState::Stopped,
                        "".to_string(),
                    ));
                    break Err(PipelineError::Interrupted(pid as u32, wstopsig(status)));
                }
                Ok(_) => (),
            }
        }
    }

    /// Waits until all running background tasks have completed, and listens for signals in the
    /// event that a signal is sent to kill the running tasks.
    pub fn wait_for_background(&mut self) -> Result<(), PipelineError> {
        while let Some(p) = self.background_jobs().iter().find(|p| p.state == ProcessState::Running)
        {
            if let Some(signal) = signals::SignalHandler.find(|&s| s != sys::SIGTSTP) {
                self.background_send(signal);
                return Err(PipelineError::Interrupted(p.pid(), signal));
            }
            sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// When given a process ID, that process's group will be assigned as the
    /// foreground process group.
    fn set_foreground_as(pid: u32) {
        signals::block();
        let _ = sys::tcsetpgrp(0, pid);
        signals::unblock();
    }

    /// Takes a background tasks's PID and whether or not it needs to be continued; resumes the
    /// task and sets it as the foreground process. Once the task exits or stops, the exit status
    /// will be returned, and ownership of the TTY given back to the shell.
    pub fn set_bg_task_in_foreground(&self, pid: u32, cont: bool) -> Status {
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
                Some(BackgroundResult::Status(stat)) => {
                    break Status::from_exit_code(i32::from(stat))
                }
                Some(BackgroundResult::Errored) => break Status::TERMINATED,
                None => sleep(Duration::from_millis(25)),
            }
        };
        // Have the shell reclaim the TTY
        Self::set_foreground_as(process::id());
        status
    }
}
