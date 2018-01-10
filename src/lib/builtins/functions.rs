use fnv::FnvHashMap;
use shell::flow_control::Function;
use shell::status::*;
use types::Identifier;

fn print_functions(functions: &FnvHashMap<Identifier, Function>) {
    println!("# Functions");
    for fn_name in functions.keys() {
        let description = &functions.get(fn_name).unwrap().get_description();
        if let Some(ref description) = *description {
            println!("    {} -- {}", fn_name, description);
        } else {
            println!("    {}", fn_name);
        }
    }
}

pub(crate) fn fn_(functions: &mut FnvHashMap<Identifier, Function>) -> i32 {
    print_functions(functions);
    SUCCESS
}
