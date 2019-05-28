use crate::lexers::{ArgumentSplitter, DesignatorLexer, DesignatorToken};
use liner::Context;
use std::{borrow::Cow, str};

pub fn expand_designators<'a>(context: &Context, cmd: &'a str) -> Cow<'a, str> {
    if let Some(buffer) = context.history.buffers.back() {
        let buffer = buffer.as_bytes();
        let buffer = unsafe { str::from_utf8_unchecked(&buffer) };
        let mut output = String::with_capacity(cmd.len());
        for token in DesignatorLexer::new(cmd.as_bytes()) {
            match token {
                DesignatorToken::Text(text) => output.push_str(text),
                DesignatorToken::Designator(text) => match text {
                    "!!" => output.push_str(buffer),
                    "!$" => output.push_str(last_arg(buffer)),
                    "!0" => output.push_str(command(buffer)),
                    "!^" => output.push_str(first_arg(buffer)),
                    "!*" => output.push_str(&args(buffer)),
                    _ => output.push_str(text),
                },
            }
        }
        Cow::Owned(output)
    } else {
        Cow::Borrowed(cmd)
    }
}

fn command(text: &str) -> &str { ArgumentSplitter::new(text).next().unwrap_or(text) }

fn args(text: &str) -> &str {
    let bytes = text.as_bytes();
    bytes
        .iter()
        // Obtain position of the first space character,
        .position(|&x| x == b' ')
        // and then obtain the arguments to the command.
        .and_then(|fp| {
            bytes[fp + 1..]
                .iter()
                // Find the position of the first character in the first argument.
                .position(|&x| x != b' ')
                // Then slice the argument string from the original command.
                .map(|sp| &text[fp + sp + 1..])
        })
        // Unwrap the arguments string if it exists, else return the original string.
        .unwrap_or(text)
}

fn first_arg(text: &str) -> &str { ArgumentSplitter::new(text).nth(1).unwrap_or(text) }

fn last_arg(text: &str) -> &str { ArgumentSplitter::new(text).last().unwrap_or(text) }
