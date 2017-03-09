use directory_stack::DirectoryStack;
use variables::Variables;
use parser::expand_string;

pub fn parse_while(expression: &str, dir_stack: &DirectoryStack, variables: &Variables) -> bool {
    match expand_string(expression, variables, dir_stack).unwrap_or_else(|_| String::from("")).as_str() {
        "1" | "true" => true,
        _ => false
    }
}
