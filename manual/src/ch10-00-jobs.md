# Job Control

## Disowning Processes

Ion features a `disown` command which supports the following flags:

- **-r**: Remove all running jobs from the background process list.
- **-h**: Specifies that each job supplied will not receive the `SIGHUP` signal when the shell
    receives a `SIGHUP`.
- **-a**: If no job IDs were supplied, remove all jobs from the background process list.

Unlike Bash, job arguments are their specified job IDs.

## Foreground & Background Tasks

When a foreground task is stopped with the **Ctrl+Z** signal, that process will be added to the
background process list as a stopped job. When a supplied command ends with the **&** operator,
this will specify to run the task the background as a running job. To resume a stopped job,
executing the `bg <job_id>` command will send a `SIGCONT` to the specified job ID, hence resuming
the job. The `fg` command will similarly do the same, but also set that task as the foreground
process. If no argument is given to either `bg` or `fg`, then the previous job will be used
as the input.

## Exiting the Shell

The `exit` command will exit the shell, sending a `SIGTERM` to any background tasks that are
still active. If no value is supplied to `exit`, then the last status that the shell received
will be used as the exit status. Otherwise, if a numeric value is given to the command, then
that value will be used as the exit status.

## Suspending the Shell

While the shell ignores `SIGTSTP` signals, you can forcefully suspend the shell by executing the
`suspend` command, which forcefully stops the shell via a `SIGSTOP` signal.
