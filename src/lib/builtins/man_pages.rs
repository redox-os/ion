use crate::types;

/// Print the given help if the -h or --help argument are found
pub fn check_help(args: &[types::Str], man_page: &'static str) -> bool {
    for arg in args {
        if arg == "-h" || arg == "--help" {
            println!("{}", man_page);
            return true;
        }
    }
    false
}

// pub const MAN_FN: &str = r#"NAME
// fn - print a list of all functions or create a function
//
// SYNOPSIS
// fn
//
// fn example arg:int
// echo $arg
// end
//
// DESCRIPTION
// fn prints a list of all functions that exist in the shell or creates a
// function when combined with the 'end' keyword. Functions can have type
// hints, to tell ion to check the type of a functions arguments. An error will
// occur if an argument supplied to a function is of the wrong type.
// The supported types in ion are, [], bool, bool[], float, float[], int,
// int[], str, str[].
//
// Functions are called by typing the function name and then the function
// arguments, separated by a space.
//
// fn example arg0:int arg1:int
// echo $arg
// end
//
// example 1
//"#;
