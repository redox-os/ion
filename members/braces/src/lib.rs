use auto_enums::auto_enum;
use permutate::Permutator;
use smallvec::SmallVec;

#[derive(Debug)]
/// A token primitive for the `expand_braces` function.
pub enum BraceToken {
    Normal(small::String),
    Expander,
}

#[auto_enum]
pub fn expand<'a>(
    tokens: &'a [BraceToken],
    expanders: &'a [&'a [&'a str]],
) -> impl Iterator<Item = small::String> + 'a {
    #[auto_enum(Iterator)]
    match expanders.len() {
        0 => ::std::iter::empty(),
        1 => SingleBraceExpand { elements: expanders[0].iter().cloned(), tokens, loop_count: 0 },
        _ => MultipleBraceExpand::new(tokens, expanders),
    }
}

fn escape_string(output: &mut SmallVec<[u8; 64]>, input: &str) {
    output.reserve(input.len());
    let mut backslash = false;
    for character in input.bytes() {
        if backslash {
            if ![b'{', b'}', b','].contains(&character) {
                output.push(b'\\');
            }
            output.push(character);
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
    buffer:     Vec<&'a str>,
}

impl<'a> MultipleBraceExpand<'a> {
    pub fn new(
        tokens: &'a [BraceToken],
        expanders: &'a [&'a [&'a str]],
    ) -> MultipleBraceExpand<'a> {
        MultipleBraceExpand {
            permutator: Permutator::new(expanders),
            tokens,
            buffer: vec![""; expanders.len()],
        }
    }
}

impl<'a> Iterator for MultipleBraceExpand<'a> {
    type Item = small::String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.permutator.next_with_buffer(&mut self.buffer) {
            let mut strings = self.buffer.iter();
            let small_vec =
                self.tokens.iter().fold(SmallVec::with_capacity(64), |mut small_vec, token| {
                    escape_string(
                        &mut small_vec,
                        match *token {
                            BraceToken::Normal(ref text) => text,
                            BraceToken::Expander => strings.next().unwrap(),
                        },
                    );
                    small_vec
                });
            Some(unsafe { small::String::from_utf8_unchecked(small_vec.to_vec()) })
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
    type Item = small::String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.loop_count {
            0 => {
                let small_vec =
                    self.tokens.iter().fold(SmallVec::with_capacity(64), |mut small_vec, token| {
                        escape_string(
                            &mut small_vec,
                            match *token {
                                BraceToken::Normal(ref text) => text,
                                BraceToken::Expander => self.elements.next().unwrap(),
                            },
                        );
                        small_vec
                    });
                self.loop_count = 1;
                Some(unsafe { small::String::from_utf8_unchecked(small_vec.to_vec()) })
            }
            _ => self.elements.next().and_then(|element| {
                let small_vec =
                    self.tokens.iter().fold(SmallVec::with_capacity(64), |mut small_vec, token| {
                        escape_string(
                            &mut small_vec,
                            match *token {
                                BraceToken::Normal(ref text) => text,
                                BraceToken::Expander => element,
                            },
                        );
                        small_vec
                    });
                Some(unsafe { small::String::from_utf8_unchecked(small_vec.to_vec()) })
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple_brace_expand() {
        let expanders: &[&[&str]] = &[&["1", "2"][..], &["3", "4"][..], &["5", "6"][..]];
        let tokens: &[BraceToken] = &[
            BraceToken::Normal("AB".into()),
            BraceToken::Expander,
            BraceToken::Normal("CD".into()),
            BraceToken::Expander,
            BraceToken::Normal("EF".into()),
            BraceToken::Expander,
            BraceToken::Normal("GH".into()),
        ];
        assert_eq!(
            MultipleBraceExpand::new(tokens, expanders).collect::<Vec<small::String>>(),
            vec![
                small::String::from("AB1CD3EF5GH"),
                small::String::from("AB1CD3EF6GH"),
                small::String::from("AB1CD4EF5GH"),
                small::String::from("AB1CD4EF6GH"),
                small::String::from("AB2CD3EF5GH"),
                small::String::from("AB2CD3EF6GH"),
                small::String::from("AB2CD4EF5GH"),
                small::String::from("AB2CD4EF6GH"),
            ]
        );
    }

    #[test]
    fn test_single_brace_expand() {
        let elements = &["one", "two", "three"];
        let tokens: &[BraceToken] = &[BraceToken::Normal("A=".into()), BraceToken::Expander];
        assert_eq!(
            SingleBraceExpand {
                elements: elements.iter().map(|element| *element),
                tokens,
                loop_count: 0
            }
            .collect::<Vec<small::String>>(),
            vec![
                small::String::from("A=one"),
                small::String::from("A=two"),
                small::String::from("A=three"),
            ]
        );
    }
}
