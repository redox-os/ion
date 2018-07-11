use small;

macro_rules! string_function {
    ($method:tt) => {
        pub fn $method(args: &[small::String]) -> i32 {
            match args.len() {
                0...2 => {
                    eprintln!("ion: {}: two arguments must be supplied", args[0]);
                    return 2;
                }
                3 => if args[1].$method(args[2].as_str()) {
                    0
                } else {
                    1
                },
                _ => if args[2..].iter().any(|arg| args[1].$method(arg.as_str())) {
                    0
                } else {
                    1
                },
            }
        }
    };
}

string_function!(starts_with);
string_function!(ends_with);
string_function!(contains);
