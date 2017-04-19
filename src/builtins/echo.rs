use std::io::{self, Write, BufWriter};

const MAN_PAGE: &'static str = /* @MANSTART{echo} */ r#"
NAME
    echo - display a line of text
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
        \\  backslash
        \a  alert (BEL)
        \b  backspace (BS)
        \c  produce no further output
        \e  escape (ESC)
        \f  form feed (FF)
        \n  new line
        \r  carriage return
        \t  horizontal tab (HT)
        \v  vertical tab (VT)
"#; /* @MANEND */

pub fn echo(args: &[String]) -> Result<(), io::Error> {
    let mut arguments: Vec<&str> = vec![];
    let mut data: Vec<&str> = vec![];

    for arg in args {
        if arg == "--help" {
            arguments.push("help");
        } else if arg.starts_with("-") {
            let arg = &arg[1..];
            for argopt in arg.chars() {
                match argopt {
                    'e' => arguments.push("escape"),
                    'n' => arguments.push("no-newline"),
                    's' => arguments.push("no-spaces"),
                    'h' => arguments.push("help"),
                    _ => (),
                }
            }
        } else {
            data.push(arg);
        }
    }

    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    if arguments.contains(&"help") {
        buffer.write_all(MAN_PAGE.as_bytes())?;
        buffer.flush()?;
        return Ok(());
    }

    let mut first = true;
    for arg in data[1..].iter().map(|x| x.as_bytes()) {
        if first {
            first = false;
        } else if !arguments.contains(&"no-spaces") {
            buffer.write_all(&[b' '])?;
        }

        if arguments.contains(&"escape") {
            let mut check = false;
            for &byte in arg {
                match byte {
                    b'\\' if check => {
                        buffer.write_all(&[byte])?;
                        check = false;
                    },
                    b'\\' => check = true,
                    b'a' if check => {
                        buffer.write_all(&[7u8])?; // bell
                        check = false;
                    },
                    b'b' if check => {
                        buffer.write_all(&[8u8])?; // backspace
                        check = false;
                    },
                    b'c' if check => {
                        buffer.flush()?;
                        return Ok(());
                    },
                    b'e' if check => {
                        buffer.write_all(&[27u8])?; // escape
                        check = false;
                    },
                    b'f' if check => {
                        buffer.write_all(&[12u8])?; // form feed
                        check = false;
                    },
                    b'n' if check => {
                        buffer.write_all(&[b'\n'])?; // newline
                        check = false;
                    },
                    b'r' if check => {
                        buffer.write_all(&[b'\r'])?;
                        check = false;
                    },
                    b't' if check => {
                        buffer.write_all(&[b'\t'])?;
                        check = false;
                    },
                    b'v' if check => {
                        buffer.write_all(&[11u8])?; // vertical tab
                        check = false;
                    },
                    _ if check => {
                        buffer.write_all(&[b'\\', byte])?;
                        check = false;
                    },
                    _ => { buffer.write_all(&[byte])?; }
                }
            }
        } else {
            buffer.write_all(arg)?;
        }
    }

    if !arguments.contains(&"no-newline") {
        buffer.write_all(&[b'\n'])?;
    }

    buffer.flush()?;

    Ok(())
}
