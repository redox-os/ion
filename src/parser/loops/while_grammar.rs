use directory_stack::DirectoryStack;
use variables::Variables;
use parser::shell_expand;

pub fn parse_while(expression: &str, dir_stack: &DirectoryStack, variables: &Variables) -> bool {
    macro_rules! expand {
        ($input:expr) => {{
            let expand_tilde = |tilde: &str| variables.tilde_expansion(tilde, dir_stack);
            let expand_variable = |variable: &str, _: bool| variables.get_var(variable);
            let expand_command = |command: &str, quoted: bool| {
                variables.command_expansion(command, quoted)
            };
            shell_expand::expand_string($input, expand_tilde, expand_variable, expand_command)
                .unwrap_or(String::from(""))
        }}
    }

    match expand!(expression).as_str() {
        "1" | "true" => true,
        _ => false
    }
}
