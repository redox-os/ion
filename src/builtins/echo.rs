use std::io::{self, BufWriter, Write};

bitflags! {
    struct Flags : u8 {
        const ESCAPE = 1;
        const NO_NEWLINE = 2;
        const NO_SPACES = 4;
    }
}

pub(crate) fn echo(args: &[&str]) -> Result<(), io::Error> {
    let mut flags = Flags::empty();
    let mut data: Vec<&str> = vec![];

    for arg in args {
        match *arg {
            "--escape" => flags |= Flags::ESCAPE,
            "--no-newline" => flags |= Flags::NO_NEWLINE,
            "--no-spaces" => flags |= Flags::NO_SPACES,
            _ => if arg.starts_with('-') {
                let mut is_opts = true;
                let opts = &arg[1..];
                let mut short_flags = Flags::empty();
                for argopt in opts.chars() {
                    match argopt {
                        'e' => short_flags |= Flags::ESCAPE,
                        'n' => short_flags |= Flags::NO_NEWLINE,
                        's' => short_flags |= Flags::NO_SPACES,
                        _ => {
                            is_opts = false;
                            break;
                        }
                    }
                }
                if is_opts {
                    flags |= short_flags;
                } else {
                    data.push(arg);
                }
            } else {
                data.push(arg);
            },
        }
    }

    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    let mut first = true;
    for arg in data[1..].iter().map(|x| x.as_bytes()) {
        if first {
            first = false;
        } else if !flags.contains(Flags::NO_SPACES) {
            buffer.write_all(&[b' '])?;
        }

        if flags.contains(Flags::ESCAPE) {
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

    if !flags.contains(Flags::NO_NEWLINE) {
        buffer.write_all(&[b'\n'])?;
    }

    buffer.flush()?;

    Ok(())
}
