use super::Status;
use small;

macro_rules! string_function {
    ($method:tt) => {
        pub fn $method(args: &[small::String], _shell: &mut crate::Shell<'_>) -> Status {
            if args.len() <= 2 {
                return Status::bad_argument(format!(
                    "ion: {}: two arguments must be supplied",
                    args[0]
                ));
            }
            if args[2..].iter().any(|arg| args[1].$method(arg.as_str())) {
                Status::SUCCESS
            } else {
                Status::error("")
            }
        }
    };
}

string_function!(starts_with);
string_function!(ends_with);
string_function!(contains);
