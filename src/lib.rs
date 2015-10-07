use std::io;

pub fn repl() {
    let mut input = String::new();
    let _unused = io::stdin().read_line(&mut input);
    println!("You typed: {}", input.trim());
}
