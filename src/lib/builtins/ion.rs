use shell::Shell;
use shell::status::*;
use std::path::Path;

use std::process::Command;

const DOCPATH: &str = "/usr/share/ion/docs/index.html";

pub(crate) fn ion_docs(_: &[&str], shell: &mut Shell) -> i32 {
    if !Path::new(DOCPATH).exists() {
        eprintln!("ion: ion shell documentation is not installed");
        return FAILURE;
    }

    if let Some(cmd) = shell.get_var("BROWSER".into()) {
        if let Ok(_) = Command::new(&cmd).arg(DOCPATH).spawn() {
            return SUCCESS;
        }
    } else {
        eprintln!("ion: BROWSER variable isn't defined");
    }

    FAILURE
}
