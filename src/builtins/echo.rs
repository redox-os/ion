extern crate coreutils;

use std::io::{self, Write};
use self::coreutils::ArgParser;

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
    let mut parser = ArgParser::new(4)
        .add_flag(&["e", "escape"])
        .add_flag(&["n", "no-newline"])
        .add_flag(&["s", "no-spaces"])
        .add_flag(&["h", "help"]);
    parser.parse(args.iter().cloned());

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    if parser.found("help") {
        stdout.write_all(MAN_PAGE.as_bytes())?;
        stdout.flush()?;
        return Ok(());
    }

    let mut first = true;
    for arg in parser.args.iter().map(|x| x.as_bytes()) {
        if first {
            first = false;
        } else if !parser.found("no-spaces") {
            stdout.write_all(&[b' '])?;
        }

        if parser.found("escape") {
            let mut check = false;
            for &byte in arg {
                match byte {
                    b'\\' if check => {
                        stdout.write_all(&[byte])?;
                        check = false;
                    },
                    b'\\' => check = true,
                    b'a' if check => {
                        stdout.write_all(&[7u8])?; // bell
                        check = false;
                    },
                    b'b' if check => {
                        stdout.write_all(&[8u8])?; // backspace
                        check = false;
                    },
                    b'c' if check => {
                        stdout.flush()?;
                        return Ok(());
                    },
                    b'e' if check => {
                        stdout.write_all(&[27u8])?; // escape
                        check = false;
                    },
                    b'f' if check => {
                        stdout.write_all(&[12u8])?; // form feed
                        check = false;
                    },
                    b'n' if check => {
                        stdout.write_all(&[b'\n'])?; // newline
                        check = false;
                    },
                    b'r' if check => {
                        stdout.write_all(&[b'\r'])?;
                        check = false;
                    },
                    b't' if check => {
                        stdout.write_all(&[b'\t'])?;
                        check = false;
                    },
                    b'v' if check => {
                        stdout.write_all(&[11u8])?; // vertical tab
                        check = false;
                    },
                    _ if check => {
                        stdout.write_all(&[b'\\', byte])?;
                        check = false;
                    },
                    _ => { stdout.write_all(&[byte])?; }
                }
            }
        } else {
            stdout.write_all(arg)?;
        }
    }

    if !parser.found("no-newline") {
        stdout.write_all(&[b'\n'])?;
    }

    stdout.flush()?;

    Ok(())
}
