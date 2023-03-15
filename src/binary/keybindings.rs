use builtins_proc::builtin_interactive;
use ion_shell::{
    builtins::{man_pages, Status},
    types, Shell,
};

use liner::{Context, KeyBindings};
use std::{cell::RefCell, rc::Rc};
#[builtin_interactive(
    desc = "changes key bindings",
    man = "
NAME
    keybindings - changes the key shortcuts to a different preset

SYNOPSIS
    keybindings [vi|emacs]

DESCRIPTION
    Set key bindings to a preset, vi or emacs.

OPTIONS:
    vi: vim keybindings
    emacs: emacs key bindings"
)]
pub fn keybindings(
    context_bis: Rc<RefCell<Context>>,
) -> impl Fn(&[types::Str], &mut Shell<'_>) -> Status {
    move |args: &[types::Str], _shell: &mut Shell<'_>| -> Status {
        if man_pages::check_help(args, HELP_PAGE) {
            return Status::SUCCESS;
        }
        match args.get(1).map(|s| s.as_str()) {
            Some("vi") => {
                context_bis.borrow_mut().key_bindings = KeyBindings::Vi;
                Status::SUCCESS
            }
            Some("emacs") => {
                context_bis.borrow_mut().key_bindings = KeyBindings::Emacs;
                Status::SUCCESS
            }
            Some(_) => Status::error("Invalid keybindings. Choices are vi and emacs"),
            None => Status::error("keybindings need an argument"),
        }
    }
}
