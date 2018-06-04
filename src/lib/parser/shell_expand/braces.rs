use super::permutate::Permutator;
use smallvec::SmallVec;

#[derive(Debug)]
/// A token primitive for the `expand_braces` function.
pub(crate) enum BraceToken {
    Normal(String),
    Expander,
}

pub(crate) fn expand_braces<'a>(
    tokens: &'a [BraceToken],
    expanders: &'a [&'a [&'a str]],
) -> Box<Iterator<Item = String> + 'a> {
    if expanders.len() > 1 {
        let multiple_brace_expand = MultipleBraceExpand::new(tokens, expanders);
        Box::new(multiple_brace_expand)
    } else if expanders.len() == 1 {
        let single_brace_expand = SingleBraceExpand {
            elements:   expanders[0].iter().map(|element| *element),
            tokens,
            loop_count: 0,
        };
        Box::new(single_brace_expand)
    } else {
        Box::new(::std::iter::empty())
    }
}

fn escape_string(output: &mut SmallVec<[u8; 64]>, input: &str) {
    let mut backslash = false;
    for character in input.bytes() {
        if backslash {
            match character {
                b'{' | b'}' | b',' => output.push(character),
                _ => {
                    output.push(b'\\');
                    output.push(character);
                }
            }
            backslash = false;
        } else if character == b'\\' {
            backslash = true;
        } else {
            output.push(character);
        }
    }
}

pub struct MultipleBraceExpand<'a> {
    permutator: Permutator<'a, str>,
    tokens:     &'a [BraceToken],
    buffer:     Vec<&'a str>
}

impl<'a> MultipleBraceExpand<'a> {
    pub(crate) fn new(
        tokens: &'a [BraceToken],
        expanders: &'a [&'a [&'a str]]
    ) -> MultipleBraceExpand<'a> {
        MultipleBraceExpand {
            permutator: Permutator::new(expanders),
            tokens,
            buffer: vec![""; expanders.len()]
        }
    }
}

impl<'a> Iterator for MultipleBraceExpand<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.permutator.next_with_buffer(&mut self.buffer) {
            let mut strings = self.buffer.iter();
            let small_vec: SmallVec<[u8; 64]> = self.tokens.iter().fold(
                SmallVec::with_capacity(64),
                |mut small_vec, token| match *token {
                    BraceToken::Normal(ref text) => {
                        escape_string(&mut small_vec, text);
                        small_vec
                    }
                    BraceToken::Expander => {
                        escape_string(&mut small_vec, strings.next().unwrap());
                        small_vec
                    }
                },
            );
            Some(unsafe { String::from_utf8_unchecked(small_vec.to_vec()) })
        } else {
            None
        }
    }
}

pub struct SingleBraceExpand<'a, 'b, I>
where
    I: Iterator<Item = &'a str>,
{
    elements:   I,
    tokens:     &'b [BraceToken],
    loop_count: usize,
}

impl<'a, 'b, I> Iterator for SingleBraceExpand<'a, 'b, I>
where
    I: Iterator<Item = &'a str>,
{
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.loop_count {
            0 => {
                let small_vec: SmallVec<[u8; 64]> = self.tokens.iter().fold(
                    SmallVec::with_capacity(64),
                    |mut small_vec, token| match *token {
                        BraceToken::Normal(ref text) => {
                            escape_string(&mut small_vec, text);
                            small_vec
                        }
                        BraceToken::Expander => {
                            escape_string(&mut small_vec, self.elements.next().unwrap());
                            small_vec
                        }
                    },
                );
                self.loop_count = 1;
                Some(unsafe { String::from_utf8_unchecked(small_vec.to_vec()) })
            }
            _ => {
                if let Some(element) = self.elements.next() {
                    let small_vec: SmallVec<[u8; 64]> = self.tokens.iter().fold(
                        SmallVec::with_capacity(64),
                        |mut small_vec, token| match *token {
                            BraceToken::Normal(ref text) => {
                                escape_string(&mut small_vec, text);
                                small_vec
                            }
                            BraceToken::Expander => {
                                escape_string(&mut small_vec, element);
                                small_vec
                            }
                        },
                    );
                    Some(unsafe { String::from_utf8_unchecked(small_vec.to_vec()) })
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
    assert_eq!(
        MultipleBraceExpand ::new(tokens, expanders).collect::<Vec<String>>(),
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
    assert_eq!(
        SingleBraceExpand {
            elements:   elements.iter().map(|element| *element),
            tokens,
            loop_count: 0,
        }.collect::<Vec<String>>(),
        vec!["A=one".to_owned(), "A=two".to_owned(), "A=three".to_owned()]
    );
}
