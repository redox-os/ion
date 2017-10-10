#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Pattern<'a> {
    StringPattern(&'a str),
    Whitespace,
}

pub(crate) fn unescape(input: String) -> String {
    let mut output = String::new();
    let mut characters = input.char_indices();
    let mut start = 0;
    while let Some((id, character)) = characters.next() {
        if character == '\\' {
            output += &input[start..id];
            if let Some((_, character)) = characters.next() {
                start = match character {
                    'n' => {output.push('\n'); id + 2},
                    '\\' => {output.push('\\'); id + 2},
                    't' => {output.push('\t'); id + 2},
                    _ => id + 1
                };
            }
        }
    }

    if start != input.len() {
        output += &input[start..];
    }
    output
}