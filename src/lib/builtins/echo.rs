use crate::types;
use smallvec::SmallVec;
use std::io::{self, BufWriter, Write};

pub fn echo(args: &[types::Str]) -> Result<(), io::Error> {
    let mut escape = false;
    let mut newline = true;
    let mut spaces = true;
    let mut data: SmallVec<[&str; 16]> = SmallVec::with_capacity(16);

    for arg in args {
        match &**arg {
            "--escape" => escape = true,
            "--no-newline" => newline = false,
            "--no-spaces" => spaces = false,
            _ if arg.starts_with('-') => {
                let mut is_opts = true;
                let opts = &arg[1..];

                let mut short_escape = false;
                let mut short_newline = true;
                let mut short_spaces = true;

                for argopt in opts.bytes() {
                    match argopt {
                        b'e' => short_escape = true,
                        b'n' => short_newline = false,
                        b's' => short_spaces = false,
                        _ => {
                            is_opts = false;
                            break;
                        }
                    }
                }
                if is_opts {
                    escape = escape || short_escape;
                    newline = newline && short_newline;
                    spaces = spaces && short_spaces;
                } else {
                    data.push(arg);
                }
            }
            _ => {
                data.push(arg);
            }
        }
    }

    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    let mut first = true;
    for arg in data[1..].iter().map(|x| x.as_bytes()) {
        if first {
            first = false;
        } else if spaces {
            buffer.write_all(&[b' '])?;
        }

        if escape {
            let mut check = false;
            for &byte in arg {
                match byte {
                    b'\\' if check => {
                        buffer.write_all(&[byte])?;
                        check = false;
                    }
                    b'\\' => check = true,
                    b'a' if check => {
                        buffer.write_all(&[7u8])?; // bell
                        check = false;
                    }
                    b'b' if check => {
                        buffer.write_all(&[8u8])?; // backspace
                        check = false;
                    }
                    b'c' if check => {
                        buffer.flush()?;
                        return Ok(());
                    }
                    b'e' if check => {
                        buffer.write_all(&[27u8])?; // escape
                        check = false;
                    }
                    b'f' if check => {
                        buffer.write_all(&[12u8])?; // form feed
                        check = false;
                    }
                    b'n' if check => {
                        buffer.write_all(&[b'\n'])?; // newline
                        check = false;
                    }
                    b'r' if check => {
                        buffer.write_all(&[b'\r'])?;
                        check = false;
                    }
                    b't' if check => {
                        buffer.write_all(&[b'\t'])?;
                        check = false;
                    }
                    b'v' if check => {
                        buffer.write_all(&[11u8])?; // vertical tab
                        check = false;
                    }
                    _ if check => {
                        buffer.write_all(&[b'\\', byte])?;
                        check = false;
                    }
                    _ => {
                        buffer.write_all(&[byte])?;
                    }
                }
            }
        } else {
            buffer.write_all(arg)?;
        }
    }

    if newline {
        buffer.write_all(&[b'\n'])?;
    }

    buffer.flush()?;

    Ok(())
}
