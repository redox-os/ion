pub fn tokenize(input: &str) -> Vec<String> {
    let mut result: Vec<String> = vec![];
    for raw_word in input.split(' ') {
        let word = raw_word.trim();
        if word.starts_with("#") {
            break;
        }
        if !word.is_empty() {
            result.push(word.to_string());
        }
    }
    result
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn tokenize_empty_string() {
        let expected: Vec<String> = vec![];
        assert_eq!(tokenize(""), expected);
    }

    #[test]
    fn tokenize_single_word() {
        let expected: Vec<String> = vec!["word".to_string()];
        assert_eq!(tokenize("word"), expected);
    }

    #[test]
    fn tokenize_whitespace() {
        let expected: Vec<String> = vec![];
        assert_eq!(tokenize(" \t   "), expected);
    }

    #[test]
    fn tokenize_multiple_words() {
        let expected: Vec<String> = vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string()];
        assert_eq!(tokenize("one two three"), expected);
    }

    #[test]
    fn tokenize_comment() {
        let expected: Vec<String> = vec![];
        assert_eq!(tokenize("# some text"), expected);
    }

    #[test]
    fn tokenize_end_of_line_comment() {
        let expected: Vec<String> = vec!["word".to_string()];
        assert_eq!(tokenize("word # more stuff"), expected);
    }
}
