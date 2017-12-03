use std::error::Error;
use std::io::{stdout, Write};

pub(crate) fn print_man(man_page: &'static str) {
    let stdout = stdout();
    let mut stdout = stdout.lock();
    match stdout.write_all(man_page.as_bytes()).and_then(|_| stdout.flush()) {
        Ok(_) => (),
        Err(err) => panic!("{}", err.description().to_owned()),
    }
}

pub(crate) fn check_help(args: &[&str], man_page: &'static str) -> bool {
    for arg in args {
        if *arg == "-h" || *arg == "--help" {
            print_man(man_page);
            return true
        }
    }
    false
}

pub(crate) const MAN_STATUS: &'static str = r#"NAME
    status - Evaluates the current runtime status

SYNOPSIS
    status [ -h | --help ] [-l] [-i]

DESCRIPTION
    With no arguments status displays the current login information of the shell.

OPTIONS
    -l
        returns true if shell is a login shell. Also --is-login.
    -i
        returns true if shell is interactive. Also --is-interactive.
    -f
        prints the filename of the currently running script or stdio. Also --current-filename.
"#;

pub(crate) const MAN_CD: &'static str = r#"NAME
    cd - Change directory.

SYNOPSIS
    cd DIRECTORY

DESCRIPTION
    Without arguments cd changes the working directory to your home directory.

    With arguments cd changes the working directory to the directory you provided.

"#;

pub(crate) const MAN_BOOL: &'static str = r#"NAME
    bool - Returns true if the value given to it is equal to '1' or 'true'.

SYNOPSIS
    bool VALUE

DESCRIPTION
    Returns true if the value given to it is equal to '1' or 'true'.
"#;

pub(crate) const MAN_IS: &'static str = r#"NAME
    is - Checks if two arguments are the same

SYNOPSIS
    is [ -h | --help ] [not]

DESCRIPTION
    Returns 0 if the two arguments are equal

OPTIONS
    not
        returns 0 if the two arguments are not equal.
"#;

pub(crate) const MAN_DIRS: &'static str = r#"NAME
    dirs - prints the directory stack

SYNOPSIS
    dirs

DESCRIPTION
    dirs prints the current directory stack.
"#;

pub(crate) const MAN_PUSHD: &'static str = r#"NAME
    pushd - push a directory to the directory stack

SYNOPSIS
    pushd DIRECTORY

DESCRIPTION
    pushd pushes a directory to the directory stack.
"#;

pub(crate) const MAN_POPD: &'static str = r#"NAME
    popd - shift through the directory stack

SYNOPSIS
    popd

DESCRIPTION
    popd removes the top directory from the directory stack and changes the working directory to the new top directory. 
    pushd adds directories to the stack.
"#;