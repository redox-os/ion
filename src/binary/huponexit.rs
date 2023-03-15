use std::{cell::Cell, rc::Rc};

use builtins_proc::builtin_interactive;
use ion_shell::{
    builtins::{man_pages, Status},
    types, Shell,
};

#[builtin_interactive(
    desc = "Toggle if it hangups the shell's background jobs on exit",
    man = "
SYNOPSIS
    huponexit [false|off]

DESCRIPTION
    If activated, it hangups the shell's background jobs on exit.
    If no arguments are provided then huponexit is activated. Can be deactivated 
    again with providing false or off.

OPTIONS:
    false or off: deactivates this behaviour"
)]
pub fn huponexit(
    huponexit_state: Rc<Cell<bool>>,
) -> impl Fn(&[types::Str], &mut Shell<'_>) -> Status {
    move |args: &[types::Str], _shell: &mut Shell<'_>| {
        if man_pages::check_help(args, HELP_PAGE) {
            return Status::SUCCESS;
        }
        huponexit_state.set(!matches!(args.get(1).map(AsRef::as_ref), Some("false") | Some("off")));
        Status::SUCCESS
    }
}
