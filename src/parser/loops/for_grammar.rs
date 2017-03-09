use directory_stack::DirectoryStack;
use variables::Variables;
use parser::expand_string;

#[derive(Debug, PartialEq)]
pub enum ForExpression {
    Normal(String),
    Range(usize, usize)
}

impl ForExpression {
    pub fn new(expression: &str, dir_stack: &DirectoryStack, variables: &Variables) -> ForExpression {
        let output = expand_string(expression, variables, dir_stack).unwrap_or_else(|_| String::from(""));

        {
            let mut bytes_iterator = output.bytes().enumerate();
            while let Some((id, byte)) = bytes_iterator.next() {
                match byte {
                    b'0'...b'9' => continue,
                    b'.' => match output[0..id].parse::<usize>().ok() {
                        Some(first_number) => {
                            let mut dots = 1;
                            for (_, byte) in bytes_iterator {
                                if byte == b'.' { dots += 1 } else { break }
                            }

                            match output[id+dots..].parse::<usize>().ok() {
                                Some(second_number) => {
                                    match dots {
                                        2 => return ForExpression::Range(first_number, second_number),
                                        3 => return ForExpression::Range(first_number, second_number+1),
                                        _ => break
                                    }
                                },
                                None => break
                            }
                        },
                        None => break
                    },
                    _ => break
                }
            }
        }

        ForExpression::Normal(output)
    }
}

#[test]
fn for_inclusive_range() {
    let dir_stack = DirectoryStack::new().unwrap();
    let variables = Variables::default();
    assert_eq!(ForExpression::new("1...10", &dir_stack, &variables), ForExpression::Range(1, 11));
}

#[test]
fn for_exclusive_range() {
    let dir_stack = DirectoryStack::new().unwrap();
    let variables = Variables::default();
    assert_eq!(ForExpression::new("1..10", &dir_stack, &variables), ForExpression::Range(1, 10));
}

#[test]
fn for_normal() {
    let dir_stack = DirectoryStack::new().unwrap();
    let variables = Variables::default();
    assert_eq!(ForExpression::new("1 2 3 4 5", &dir_stack, &variables), ForExpression::Normal("1 2 3 4 5".to_string()));
}

#[test]
fn for_variable() {
    let dir_stack = DirectoryStack::new().unwrap();
    let mut variables = Variables::default();
    variables.set_var("A", "1 2 3 4 5");
    assert_eq!(ForExpression::new("$A", &dir_stack, &variables), ForExpression::Normal("1 2 3 4 5".to_string()));
}
