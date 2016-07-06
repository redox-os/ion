use std::io::stdin;

pub fn readln() -> Option<String> {
    let mut buffer = String::new();
    stdin().read_line(&mut buffer).ok().map_or(None, |_| Some(buffer))
}
