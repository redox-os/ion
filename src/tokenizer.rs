#[derive(PartialEq)]
#[derive(Debug)]
pub enum Token {
    Word(String),
    End,
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut result: Vec<Token> = vec![];
    for raw_word in input.split(' ') {
        let word = raw_word.trim();
        if word.starts_with("#") {
            break;
        }
        if !word.is_empty() {
            result.push(Token::Word(word.to_string()));
        }
    }
    result
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn tokenize_empty_string() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn tokenize_single_word() {
        let expected: Vec<Token> = vec![Token::Word("word".to_string())];
        assert_eq!(expected, tokenize("word"));
    }

    #[test]
    fn tokenize_whitespace() {
        assert!(tokenize(" \t   ").is_empty());
    }

    #[test]
    fn tokenize_multiple_words() {
        let expected: Vec<Token> = vec![
            Token::Word("one".to_string()),
            Token::Word("two".to_string()),
            Token::Word("three".to_string())];
        assert_eq!(expected, tokenize("one two three"));
    }

    #[test]
    fn tokenize_comment() {
        assert!(tokenize("# some text").is_empty());
    }

    #[test]
    fn tokenize_end_of_line_comment() {
        let expected: Vec<Token> = vec![Token::Word("word".to_string())];
        assert_eq!(expected, tokenize("word # more stuff"));
    }
}
