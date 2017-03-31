use flow_control::{Function};
use std::collections::HashMap;
use status::*;
use std::io::{self, Write};

fn print_functions(functions: &HashMap<String, Function>) {
    let stdout = io::stdout();
    let stdout = &mut stdout.lock();

    let _ = writeln!(stdout, "# Functions");
    for fn_name in functions.keys() {
        let _ = writeln!(stdout, "    {}", fn_name);
    }
}

pub fn fn_(functions: &mut HashMap<String, Function>) -> i32
{
    print_functions(functions);
    SUCCESS
}
