use super::permutate::Permutator;
use smallvec::SmallVec;
use std::iter::Extend;

#[derive(Debug)]
/// A token primitive for the `expand_braces` function.
pub(crate) enum BraceToken {
    Normal(String),
    Expander,
}

pub(crate) fn expand_braces<'a>(tokens: &'a [BraceToken], mut expanders: Vec<Vec<String>>) -> Box<Iterator<Item = String> + 'a> {
    if expanders.len() > 1 {
        let tmp: Vec<Vec<&str>> = expanders
            .iter()
            .map(|list| list.iter().map(AsRef::as_ref).collect::<Vec<&str>>())
            .collect();
        let vector_of_arrays: Vec<&[&str]> = tmp.iter().map(AsRef::as_ref).collect();
        let multiple_brace_expand = MultipleBraceExpand {
                permutator: Permutator::new(&vector_of_arrays),
                tokens: tokens,
            };
        Box::new(multiple_brace_expand)
    } else if expanders.len() == 1 {
        let elements = expanders
            .drain(..)
            .next()
            .expect("there should be at least one value");
        let single_brace_expand = SingleBraceExpand {
                elements: elements.iter().map(AsRef::as_ref),
                tokens: tokens,
                loop_count: 0,
            };
        Box::new(single_brace_expand)
    } else {
        Box::new(::std::iter::empty())
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

pub struct MultipleBraceExpand<'a, 'b> {
    permutator: Permutator<'a, str>,
    tokens: &'b [BraceToken],
}

impl<'a, 'b> Iterator for MultipleBraceExpand<'a, 'b> {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(permutation) = self.permutator.next() {
            let mut strings = permutation.iter();
            let small_vec: SmallVec<[u8; 64]> = self.tokens.iter().fold(SmallVec::with_capacity(64), |mut small_vec, token| {
                match *token {
                    BraceToken::Normal(ref text) => {
                        small_vec.extend(escape_string(text).bytes());
                        small_vec
                    }
                    BraceToken::Expander => {
                        small_vec.extend(escape_string(strings.next().unwrap()).bytes());
                        small_vec
                    }
                }
            });
            Some(unsafe {String::from_utf8_unchecked(small_vec.to_vec())})
        } else {
            None
        }
    }
}

pub struct SingleBraceExpand<'a, 'b, I>
    where I: Iterator<Item = &'a str>
{
    elements: I,
    tokens: &'b [BraceToken],
    loop_count: usize,
}

impl<'a, 'b, I> Iterator for SingleBraceExpand<'a, 'b, I>
    where I: Iterator<Item = &'a str>
{
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        match self.loop_count {
            0 => {
                let small_vec: SmallVec<[u8; 64]> = self.tokens.iter().fold(SmallVec::with_capacity(64), |mut small_vec, token| {
                    match *token {
                        BraceToken::Normal(ref text) => {
                            small_vec.extend(escape_string(text).bytes());
                            small_vec
                        }
                        BraceToken::Expander => {
                            small_vec.extend(escape_string(self.elements.next().unwrap()).bytes());
                            small_vec
                        }
                    }
                });
                self.loop_count = 1;
                Some(unsafe {String::from_utf8_unchecked(small_vec.to_vec())})
            }
            _ => {
                if let Some(element) = self.elements.next() {
                    let small_vec: SmallVec<[u8; 64]> = self.tokens.iter().fold(SmallVec::with_capacity(64), |mut small_vec, token| {
                        match *token {
                            BraceToken::Normal(ref text) => {
                                small_vec.extend(escape_string(text).bytes());
                                small_vec
                            }
                            BraceToken::Expander => {
                                small_vec.extend(escape_string(element).bytes());
                                small_vec
                            }
                        }
                    });
                    Some(unsafe {String::from_utf8_unchecked(small_vec.to_vec())})
                } else {
                    None
                }
            }
        }
    }
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
    assert_eq!(
        out,
        vec!["A=one".to_owned(), "A=two".to_owned(), "A=three".to_owned()]
    );
}
