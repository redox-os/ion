use fnv::FnvHashMap;
use shell::flow_control::Function;
use shell::status::*;
use std::io::{self, Write};
use types::Identifier;

fn print_functions(functions: &FnvHashMap<Identifier, Function>) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();
    let _ = writeln!(stdout, "# Functions");
    for fn_name in functions.keys() {
        let description = &functions.get(fn_name).unwrap().description;
        if let Some(ref description) = *description {
            let _ = writeln!(stdout, "    {} -- {}", fn_name, description);
        } else {
            let _ = writeln!(stdout, "    {}", fn_name);
        }
    }
}

pub(crate) fn fn_(functions: &mut FnvHashMap<Identifier, Function>) -> i32 {
    print_functions(functions);
    SUCCESS
}
