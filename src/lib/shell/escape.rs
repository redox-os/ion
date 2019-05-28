use crate::{
    shell::directory_stack::DirectoryStack,
    sys::{env as sys_env, variables as self_sys},
};
use std::{borrow::Cow, env};

/// Escapes filenames from the completer so that special characters will be properly escaped.
///
/// NOTE: Perhaps we should submit a PR to Liner to add a &'static [u8] field to
/// `FilenameCompleter` so that we don't have to perform the escaping ourselves?
pub fn escape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    for character in input.bytes() {
        match character {
            b'(' | b')' | b'[' | b']' | b'&' | b'$' | b'@' | b'{' | b'}' | b'<' | b'>' | b';'
            | b'"' | b'\'' | b'#' | b'^' | b'*' => output.push(b'\\'),
            _ => (),
        }
        output.push(character);
    }
    unsafe { String::from_utf8_unchecked(output) }
}

/// Unescapes filenames to be passed into the completer
pub fn unescape(input: &str) -> Cow<str> {
    let mut input: Cow<str> = input.into();
    while let Some(found) = input.find('\\') {
        if input.as_ref().len() > found + 1 {
            input.to_mut().remove(found);
        } else {
            break;
        }
    }
    input
}

pub fn tilde(input: &str, dir_stack: &DirectoryStack, prev: Option<&str>) -> Option<String> {
    // Only if the first character is a tilde character will we perform expansions
    if !input.starts_with('~') {
        return None;
    }

    let separator = input[1..].find(|c| c == '/' || c == '$');
    let (tilde_prefix, rest) = input[1..].split_at(separator.unwrap_or(input.len() - 1));

    match tilde_prefix {
        "" => sys_env::home_dir().map(|home| home.to_string_lossy().to_string() + rest),
        "+" => Some(env::var("PWD").unwrap_or_else(|_| "?".to_string()) + rest),
        "-" => prev.map(|oldpwd| oldpwd.to_string() + rest),
        _ => {
            let (neg, tilde_num) = if tilde_prefix.starts_with('+') {
                (false, &tilde_prefix[1..])
            } else if tilde_prefix.starts_with('-') {
                (true, &tilde_prefix[1..])
            } else {
                (false, tilde_prefix)
            };

            match tilde_num.parse() {
                Ok(num) => {
                    if neg { dir_stack.dir_from_top(num) } else { dir_stack.dir_from_bottom(num) }
                        .map(|path| path.to_str().unwrap().to_string())
                }
                Err(_) => self_sys::get_user_home(tilde_prefix).map(|home| home + rest),
            }
        }
    }
}
