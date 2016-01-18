use std::collections::BTreeMap;
use super::tokenizer::Token;

pub fn expand_tokens(tokens: &mut Vec<Token>, variables: &BTreeMap<String, String>) -> Vec<Token> {
    let mut expanded_tokens: Vec<Token> = vec![];
    for token in tokens.drain(..) {
        expanded_tokens.push(match token {
            Token::Word(word) => {
                if word.starts_with('$') {
                    let key = word[1..word.len()].to_string();
                    if let Some(value) = variables.get(&key) {
                        Token::Word(value.clone())
                    } else {
                        Token::Word(String::new())
                    }
                } else {
                    Token::Word(word)
                }
            }
            _ => token,
        });
    }
    expanded_tokens
}
