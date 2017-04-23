use shell::variables::Variables;

#[derive(Debug, PartialEq, Clone)]
pub enum Binding {
    InvalidKey(String),
    ListEntries,
    KeyOnly(String),
    KeyValue(String, String),
    Math(String, Operator, String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Operator {
    Add,
    Subtract,
    Divide,
    Multiply,
    Exponent,
}

/// Parses let bindings, `let VAR = KEY`, returning the result as a `(key, value)` tuple.
pub fn parse_assignment(arguments: &str) -> Binding {
    // Create a character iterator from the arguments.
    let mut char_iter = arguments.chars();

    // Find the key and advance the iterator until the equals operator is found.
    let mut key = "".to_owned();
    let mut found_key = false;
    let mut operator = None;

    macro_rules! match_operator {
        ($op:expr) => {
            if char_iter.next() == Some('=') {
                operator = Some($op);
                found_key = true;
            }
        }
    }

    // Scans through characters until the key is found, then continues to scan until
    // the equals operator is found.
    while let Some(character) = char_iter.next() {
        match character {
            ' ' if key.is_empty() => (),
            ' ' => found_key = true,
            '+' => {
                match_operator!(Operator::Add);
                break
            },
            '-' => {
                match_operator!(Operator::Subtract);
                break
            },
            '*' => {
                match_operator!(Operator::Multiply);
                break
            },
            '/' => {
                match_operator!(Operator::Divide);
                break
            },
            '^' => {
                match_operator!(Operator::Exponent);
                break
            },
            '=' => {
                found_key = true;
                break
            },
            _ if !found_key => key.push(character),
            _ => ()
        }
    }

    if !found_key && key.is_empty() {
        Binding::ListEntries
    } else {
        let value = char_iter.skip_while(|&x| x == ' ').collect::<String>();
        if value.is_empty() {
            Binding::KeyOnly(key)
        } else if !Variables::is_valid_variable_name(&key) {
            Binding::InvalidKey(key)
        } else {
            match operator {
                Some(operator) => Binding::Math(key, operator, value),
                None => Binding::KeyValue(key, value)
            }
        }
    }
}
