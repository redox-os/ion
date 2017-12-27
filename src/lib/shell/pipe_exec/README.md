# Pipeline Execution Module

The purpose of the pipeline execution module is to create commands from supplied pieplines, and
manage their execution thereof. That includes forking, executing commands, managing process group
IDs, watching foreground and background tasks, sending foreground tasks to the background,
handling pipeline and conditional operators, and std{in,out,err} redirections.
