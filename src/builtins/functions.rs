use flow_control::{Function};
use fnv::FnvHashMap;
use status::*;
use std::io::{self, Write};

fn print_functions(functions: &FnvHashMap<String, Function>) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();
    let _ = writeln!(stdout, "# Functions");
    for fn_name in functions.keys() {
        let description = &functions.get(fn_name).unwrap().description;
        if description.len() >= 1 {
            let _ = writeln!(stdout, "    {} -- {}", fn_name, description);
        } else {
            let _ = writeln!(stdout, "    {}", fn_name);
        }
    }
}

pub fn fn_(functions: &mut FnvHashMap<String, Function>) -> i32
{
    print_functions(functions);
    SUCCESS
}
