use shell::{status::*, Shell};

macro_rules! string_function {
    ($method:tt) => {
        pub(crate) fn $method(args: &[&str], _: &mut Shell) -> i32 {
            match args.len() {
                0...2 => {
                    eprintln!("ion: {}: two arguments must be supplied", args[0]);
                    return BAD_ARG;
                }
                3 => if args[1].$method(&args[2]) {
                    SUCCESS
                } else {
                    FAILURE
                },
                _ => {
                    for arg in args[2..].iter() {
                        if args[1].$method(arg) {
                            return SUCCESS;
                        }
                    }
                    FAILURE
                }
            }
        }
    };
}

string_function!(starts_with);
string_function!(ends_with);
string_function!(contains);
