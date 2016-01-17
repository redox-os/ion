use std::io::stdin;

pub fn readln() -> Option<String> {
    let mut buffer = String::new();
    match stdin().read_line(&mut buffer) {
        Ok(_) => Some(buffer),
        Err(_) => None,
    }
}
