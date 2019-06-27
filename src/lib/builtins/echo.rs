use super::Status;
use crate as ion_shell;
use crate::{types, Shell};
use builtins_proc::builtin;
use smallvec::SmallVec;
use std::io::{self, BufWriter, Write};

#[builtin(
    desc = "display text",
    man = "
SYNOPSIS
    echo [ -h | --help ] [-e] [-n] [-s] [STRING]...

DESCRIPTION
    Print the STRING(s) to standard output.

OPTIONS
    -e
        enable the interpretation of backslash escapes
    -n
        do not output the trailing newline
    -s
        do not separate arguments with spaces

    Escape Sequences
        When the -e argument is used, the following sequences will be interpreted:
        \\\\  backslash
        \\a  alert (BEL)
        \\b  backspace (BS)
        \\c  produce no further output
        \\e  escape (ESC)
        \\f  form feed (FF)
        \\n  new line
        \\r  carriage return
        \\t  horizontal tab (HT)
        \\v  vertical tab (VT)"
)]
pub fn echo(args: &[types::Str], _: &mut Shell<'_>) -> Status {
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

    let mut inner = || -> std::io::Result<()> {
        let mut first = true;
        for arg in data[1..].iter().map(|x| x.as_bytes()) {
            if spaces && !first {
                buffer.write_all(b" ")?;
            }
            first = false;

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
                            buffer.write_all(&[7])?; // bell
                            check = false;
                        }
                        b'b' if check => {
                            buffer.write_all(&[8])?; // backspace
                            check = false;
                        }
                        b'c' if check => {
                            return Ok(());
                        }
                        b'e' if check => {
                            buffer.write_all(&[27])?; // escape
                            check = false;
                        }
                        b'f' if check => {
                            buffer.write_all(&[12])?; // form feed
                            check = false;
                        }
                        b'n' if check => {
                            buffer.write_all(b"\n")?; // newline
                            check = false;
                        }
                        b'r' if check => {
                            buffer.write_all(b"\r")?;
                            check = false;
                        }
                        b't' if check => {
                            buffer.write_all(b"\t")?;
                            check = false;
                        }
                        b'v' if check => {
                            buffer.write_all(&[11])?; // vertical tab
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
        Ok(())
    };

    inner().and_then(|_| buffer.flush()).into()
}
