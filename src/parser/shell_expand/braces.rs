use super::permutate::Permutator;

#[derive(Debug)]
/// A token primitive for the `expand_braces` function.
pub(crate) enum BraceToken {
    Normal(String),
    Expander,
}

pub(crate) fn expand_braces(tokens: &[BraceToken], mut expanders: Vec<Vec<String>>) -> Vec<String> {
    if expanders.len() > 1 {
        let tmp: Vec<Vec<&str>> = expanders
            .iter()
            .map(|list| list.iter().map(AsRef::as_ref).collect::<Vec<&str>>())
            .collect();
        let vector_of_arrays: Vec<&[&str]> = tmp.iter().map(AsRef::as_ref).collect();
        multiple_brace_expand(&vector_of_arrays[..], tokens)
    } else if expanders.len() == 1 {
        let elements = expanders.drain(..).next().expect("there should be at least one value");
        let elements: Vec<&str> = elements.iter().map(AsRef::as_ref).collect();
        single_brace_expand(&elements, tokens)
    } else {
        Vec::new()
    }
}

fn escape_string(input: &str) -> String {
    let mut output = String::new();
    let mut backslash = false;
    for character in input.chars() {
        if backslash {
            match character {
                '{' | '}' | ',' => output.push(character),
                _ => {
                    output.push('\\');
                    output.push(character);
                }
            }
            backslash = false;
        } else if character == '\\' {
            backslash = true;
        } else {
            output.push(character);
        }
    }
    output
}

fn multiple_brace_expand(expanders: &[&[&str]], tokens: &[BraceToken]) -> Vec<String> {
    let mut permutations = Permutator::new(expanders);
    let mut words = Vec::new();
    let mut string = String::new();

    {
        let permutation = permutations.next().unwrap();
        let mut permutations = permutation.iter();
        for token in tokens {
            match *token {
                BraceToken::Normal(ref text) => string.push_str(&escape_string(text)),
                BraceToken::Expander => {
                    string.push_str(&escape_string(permutations.next().unwrap()))
                }
            }
        }
        words.push(string.clone());
        string.clear();
    }

    for permutation in permutations {
        let mut permutations = permutation.iter();
        for token in tokens {
            match *token {
                BraceToken::Normal(ref text) => string.push_str(&escape_string(text)),
                BraceToken::Expander => {
                    string.push_str(&escape_string(permutations.next().unwrap()))
                }
            }
        }
        words.push(string.clone());
        string.clear();
    }

    words
}

fn single_brace_expand(elements: &[&str], tokens: &[BraceToken]) -> Vec<String> {
    let mut elements = elements.iter();
    let mut words = Vec::new();
    let mut string = String::new();

    for token in tokens {
        match *token {
            BraceToken::Normal(ref text) => string.push_str(&escape_string(text)),
            BraceToken::Expander => string.push_str(&escape_string(elements.next().unwrap())),
        }
    }
    words.push(string.clone());
    string.clear();

    for element in elements {
        for token in tokens {
            match *token {
                BraceToken::Normal(ref text) => string.push_str(&escape_string(text)),
                BraceToken::Expander => string.push_str(&escape_string(element)),
            }
        }
        words.push(string.clone());
        string.clear();
    }

    words
}

#[test]
fn test_multiple_brace_expand() {
    let expanders: &[&[&str]] = &[&["1", "2"][..], &["3", "4"][..], &["5", "6"][..]];
    let tokens: &[BraceToken] = &[
        BraceToken::Normal("AB".to_owned()),
        BraceToken::Expander,
        BraceToken::Normal("CD".to_owned()),
        BraceToken::Expander,
        BraceToken::Normal("EF".to_owned()),
        BraceToken::Expander,
        BraceToken::Normal("GH".to_owned()),
    ];
    let out = multiple_brace_expand(expanders, tokens);
    assert_eq!(
        out,
        vec![
            "AB1CD3EF5GH".to_owned(),
            "AB1CD3EF6GH".to_owned(),
            "AB1CD4EF5GH".to_owned(),
            "AB1CD4EF6GH".to_owned(),
            "AB2CD3EF5GH".to_owned(),
            "AB2CD3EF6GH".to_owned(),
            "AB2CD4EF5GH".to_owned(),
            "AB2CD4EF6GH".to_owned(),
        ]
    );
}

#[test]
fn test_single_brace_expand() {
    let elements = &["one", "two", "three"];
    let tokens: &[BraceToken] = &[BraceToken::Normal("A=".to_owned()), BraceToken::Expander];
    let out = single_brace_expand(elements, &tokens);
    assert_eq!(out, vec!["A=one".to_owned(), "A=two".to_owned(), "A=three".to_owned()]);
}
