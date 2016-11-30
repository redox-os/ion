use super::permutate::Permutator;
/// A token primitive for the `expand_braces` function.
enum BraceToken<'a> { Normal(&'a str), Expander }

/// When supplied with an `input` string which contains a single word that requires brace expansion, the string will be
/// tokenized into a collection of `BraceToken`s, separating the expanders from the normal text. Each expander's
/// associated elements will be collected separately and permutated together before being finally integrated into a
/// single output string.
pub fn expand_braces(output: &mut String, input: &str) {
    let mut tokens:           Vec<BraceToken> = Vec::new();
    let mut expanders:        Vec<Vec<&str>>  = Vec::new();
    let mut current_expander: Vec<&str>       = Vec::new();
    let mut start                             = 0;
    let mut expander_found                    = false;
    let mut backslash                         = false;

    for (id, character) in input.chars().enumerate() {
        if backslash {
            backslash = false;
        } else if character == '\\' {
            backslash = true;
        } else if expander_found {
            if character == '}' {
                expander_found = false;
                current_expander.push(&input[start..id]);
                start = id+1;
                tokens.push(BraceToken::Expander);
                expanders.push(current_expander.clone());
                current_expander.clear();
            } else if character == ',' {
                current_expander.push(&input[start..id]);
                start = id+1;
            }
        } else if character == '{' {
            expander_found = true;
            if id != start {
                tokens.push(BraceToken::Normal(&input[start..id]));
            }
            start = id+1;
        }
    }
    if start != input.len() {
        tokens.push(BraceToken::Normal(&input[start..]));
    }

    if expanders.len() > 1 {
        let vector_of_arrays: Vec<&[&str]> = expanders.iter().map(AsRef::as_ref).collect();
        multiple_brace_expand(output, &vector_of_arrays[..], &tokens);
    } else if expanders.len() == 1 {
        let elements = &expanders[0];
        single_brace_expand(output, elements, &tokens);
    }
}

#[test]
fn escaped_braces() {
    let mut output = String::new();
    expand_braces(&mut output, "e\\{sdf{1\\{2,34}");
    assert_eq!(output, "e{sdf1{2 e{sdf34");
}

#[test]
fn test_expand_brace_permutations() {
    let mut actual = String::new();
    expand_braces(&mut actual, "AB{12,34}CD{4,5}EF");
    let expected = String::from("AB12CD4EF AB12CD5EF AB34CD4EF AB34CD5EF");
    assert_eq!(actual, expected);
}

#[test]
fn test_expand_brace() {
    let mut actual = String::new();
    expand_braces(&mut actual, "AB{12,34}EF");
    let expected = String::from("AB12EF AB34EF");
    assert_eq!(actual, expected);
    let mut actual = String::new();
    expand_braces(&mut actual, "A{12,34}");
    let expected = String::from("A12 A34");
    assert_eq!(actual, expected);
}

fn escape_string(output: &mut String, input: &str) {
    let mut backslash = false;
    for character in input.chars() {
        if backslash {
            match character {
                '{' | '}' | ',' => output.push(character),
                _   => {
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
}

fn multiple_brace_expand(out: &mut String, expanders: &[&[&str]], tokens: &[BraceToken]) {
    let mut permutations = Permutator::new(expanders);

    {
        let permutation = permutations.next().unwrap();
        let mut permutations = permutation.iter();
        for token in tokens {
            match *token {
                BraceToken::Normal(text) => escape_string(out, text),
                BraceToken::Expander     => escape_string(out, permutations.next().unwrap()),
            }
        }
    }

    for permutation in permutations {
        out.push(' ');
        let mut permutations = permutation.iter();
        for token in tokens {
            match *token {
                BraceToken::Normal(text) => escape_string(out, text),
                BraceToken::Expander     => escape_string(out, permutations.next().unwrap()),
            }
        }
    }
}

fn single_brace_expand(out: &mut String, elements: &[&str], tokens: &[BraceToken]) {
    let mut elements = elements.iter();

    for token in tokens {
        match *token {
            BraceToken::Normal(text) => escape_string(out, text),
            BraceToken::Expander     => escape_string(out, elements.next().unwrap()),
        }
    }

    for element in elements {
        out.push(' ');
        for token in tokens {
            match *token {
                BraceToken::Normal(text) => escape_string(out, text),
                BraceToken::Expander     => escape_string(out, element),
            }
        }
    }
}

#[test]
fn test_multiple_brace_expand() {
    let mut out = String::new();
    let expanders: &[&[&str]] = &[
        &["1", "2"][..],
        &["3", "4"][..],
        &["5", "6"][..],
    ];
    let tokens: &[BraceToken] = &[
        BraceToken::Normal("AB"), BraceToken::Expander,     BraceToken::Normal("CD"),
        BraceToken::Expander,     BraceToken::Normal("EF"), BraceToken::Expander,
        BraceToken::Normal("GH")
    ];
    multiple_brace_expand(&mut out, expanders, tokens);
    assert_eq!(&out, "AB1CD3EF5GH AB1CD3EF6GH AB1CD4EF5GH AB1CD4EF6GH AB2CD3EF5GH AB2CD3EF6GH AB2CD4EF5GH AB2CD4EF6GH");

}

#[test]
fn test_single_brace_expand() {
    let mut out = String::new();
    let elements = &["one", "two", "three"];
    let tokens: &[BraceToken] = &[BraceToken::Normal("A="), BraceToken::Expander];
    single_brace_expand(&mut out, elements, &tokens);
    assert_eq!(&out, "A=one A=two A=three");
}
