use crate::shell::{status::Status, variables::Variables};
use std::io::{self, Write};

pub fn print_functions(vars: &Variables<'_>) -> Status {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();
    let _ = writeln!(stdout, "# Functions");
    for (fn_name, function) in vars.functions() {
        let description = function.get_description();
        if let Some(ref description) = description {
            let _ = writeln!(stdout, "    {} -- {}", fn_name, description);
        } else {
            let _ = writeln!(stdout, "    {}", fn_name);
        }
    }
    Status::SUCCESS
}
