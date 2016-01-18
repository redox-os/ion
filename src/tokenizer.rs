#[derive(Debug, PartialEq)]
pub enum Token {
    Word(String),
    End,
}

#[derive(Debug, PartialEq)]
enum TokenizerState {
    Default,
    DoubleQuoted,
    SingleQuoted,
    Commented,
}

fn process_character_double_quoted(_: &mut Vec<Token>,
                                   current_token: &mut String,
                                   chr: char)
                                   -> TokenizerState {
    match chr {
        '"' => TokenizerState::Default,
        _ => {
            current_token.push(chr);
            TokenizerState::DoubleQuoted
        }
    }
}

fn process_character_single_quoted(_: &mut Vec<Token>,
                                   current_token: &mut String,
                                   chr: char)
                                   -> TokenizerState {
    match chr {
        '\'' => TokenizerState::Default,
        _ => {
            current_token.push(chr);
            TokenizerState::SingleQuoted
        }
    }
}

fn process_character_comment(tokens: &mut Vec<Token>, _: &mut String, chr: char) -> TokenizerState {
    match chr {
        '\n' | '\r' => {
            tokens.push(Token::End);
            TokenizerState::Default
        }
        _ => TokenizerState::Commented,
    }
}

fn process_character_default(tokens: &mut Vec<Token>,
                             current_token: &mut String,
                             chr: char)
                             -> TokenizerState {
    let mut next_state = TokenizerState::Default;
    match chr {
        ' ' | '\t' => {
            if !current_token.is_empty() {
                tokens.push(Token::Word(current_token.clone()));
                current_token.clear();
            }
        }
        '#' => {
            next_state = TokenizerState::Commented;
        }
        '\n' | '\r' | ';' => {
            if !current_token.is_empty() {
                tokens.push(Token::Word(current_token.clone()));
                current_token.clear();
            }
            tokens.push(Token::End);
        }
        '"' => {
            next_state = TokenizerState::DoubleQuoted;
        }
        '\'' => {
            next_state = TokenizerState::SingleQuoted;
        }
        _ => {
            current_token.push(chr);
        }
    }
    next_state
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut state = TokenizerState::Default;
    let mut tokens: Vec<Token> = vec![];
    let mut current_token: String = String::new();
    for chr in input.chars() {
        state = match state {
            TokenizerState::DoubleQuoted => {
                process_character_double_quoted(&mut tokens, &mut current_token, chr)
            }
            TokenizerState::SingleQuoted => {
                process_character_single_quoted(&mut tokens, &mut current_token, chr)
            }
            TokenizerState::Commented => {
                process_character_comment(&mut tokens, &mut current_token, chr)
            }
            _ => process_character_default(&mut tokens, &mut current_token, chr),
        }
    }
    if !current_token.is_empty() {
        tokens.push(Token::Word(current_token.clone()));
    }
    tokens.push(Token::End);
    tokens
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn tokenize_empty_string() {
        let expected = vec![Token::End];
        assert_eq!(expected, tokenize(""));
    }

    #[test]
    fn tokenize_single_word() {
        let expected = vec![Token::Word("word".to_string()), Token::End];
        assert_eq!(expected, tokenize("word"));
    }

    #[test]
    fn tokenize_whitespace() {
        let expected = vec![Token::End];
        assert_eq!(expected, tokenize(" \t   "));
    }

    #[test]
    fn tokenize_multiple_words() {
        let expected = vec![Token::Word("one".to_string()),
                            Token::Word("two".to_string()),
                            Token::Word("three".to_string()),
                            Token::End];
        assert_eq!(expected, tokenize("one two three"));
    }

    #[test]
    fn tokenize_comment() {
        let expected = vec![Token::End];
        assert_eq!(expected, tokenize("# some text"));
    }

    #[test]
    fn tokenize_end_of_line_comment() {
        let expected = vec![Token::Word("word".to_string()), Token::End];
        assert_eq!(expected, tokenize("word # more stuff"));
    }

    #[test]
    fn tokenize_newline_produces_end_token() {
        let expected = vec![Token::Word("word".to_string()), Token::End];
        assert_eq!(expected, tokenize("word"));
    }

    #[test]
    fn double_quotes_escape_space() {
        let expected = vec![Token::Word("escaped space".to_string()), Token::End];
        assert_eq!(expected, tokenize("\"escaped space\""));
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        let expected = vec![Token::Word("one".to_string()),
                            Token::Word("two# three".to_string()),
                            Token::Word("four".to_string()),
                            Token::End];
        assert_eq!(expected, tokenize("one \"two# three\" four"));
    }

    #[test]
    fn mixed_double_and_single_quotes() {
        let expected = vec![Token::Word("''".to_string()),
                            Token::Word("\"\"".to_string()),
                            Token::End];
        assert_eq!(expected, tokenize("\"''\" '\"\"'"));
    }

    #[test]
    fn comment_before_newline() {
        let expected = vec![Token::Word("ls".to_string()),
                            Token::End,
                            Token::Word("help".to_string()),
                            Token::End];
        assert_eq!(expected, tokenize("ls # look in dir\nhelp"));
    }
}
