/// Escapes filenames from the completer so that special characters will be properly escaped.
///
/// NOTE: Perhaps we should submit a PR to Liner to add a &'static [u8] field to
/// `FilenameCompleter` so that we don't have to perform the escaping ourselves?
pub(crate) fn escape(input: &str) -> String {
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
pub(crate) fn unescape(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        match b {
            b'\\' => if let Some(next) = bytes.next() {
                output.push(next);
            } else {
                output.push(b'\\')
            },
            _ => output.push(b),
        }
    }
    unsafe { String::from_utf8_unchecked(output) }
}
