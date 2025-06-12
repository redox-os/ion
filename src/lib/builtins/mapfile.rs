use std::{
    error::Error,
    io::{BufRead, Read},
};

use super::Status;
use crate::{self as ion_shell, builtins::EmptyCompleter, shell::Shell, types};
use buf_read_splitter as brs;
use builtins_proc::builtin;
use liner::{Context, Prompt};
use types_rs::{types::Array, Value};

const BSI_BUFFER_SIZE: usize = 128;

struct BufSplitterIterator<'a, T: brs::Matcher> {
    reader:   brs::BufReadSplitter<'a, T>,
    buffer:   [u8; BSI_BUFFER_SIZE],
    all_read: bool,
}

impl<'a, T: brs::Matcher> Iterator for BufSplitterIterator<'a, T> {
    type Item = std::result::Result<String, Box<dyn Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.all_read {
            return None;
        }
        self.buffer.fill(0);
        let mut s = String::new();

        loop {
            let sz = self.reader.read(&mut self.buffer);
            match sz {
                Ok(sz) => match sz {
                    0 => {
                        self.all_read = match self.reader.next_part() {
                            Ok(v) => v.is_none(),
                            Err(e) => return Some(Err(e)),
                        };
                        return Some(Ok(s));
                    }
                    _ => {
                        s.push_str(&String::from_utf8_lossy(&self.buffer[..sz]));
                    }
                },

                Err(e) => return Some(Err(Box::new(e))),
            }
        }
    }
}

impl<'a, T: brs::Matcher> BufSplitterIterator<'a, T> {
    pub fn new(reader: brs::BufReadSplitter<'a, T>) -> Self {
        Self { reader, buffer: [0; BSI_BUFFER_SIZE], all_read: false }
    }
}

#[builtin(
    desc = "Split files/stdin according to a delimiter",
    man = "
SYNOPSIS
    mapfile [-h | --help] [-d DELIMITER] [-n COUNT] [-O ORIGIN] [-s COUNT] [-u FILENAME] \
           [--keep-empty] [ARRAY_NAME]

DESCRIPTION
    Read lines from the standard input into the indexed array variable array, or from a file if \
           the -u option is supplied and split them with new line.
    MAPFILE is the default variable name.

OPTIONS
    -d [DELIMITER]
        Split the input with the DELIMITER instead of new lines.
        The delimiter may be several characters
    --keep-empty 
        By default empty 'lines' are removed, this option enables the storage of empty 'lines'
    -n [COUNT]
        Read at most COUNT 'lines' from the input, default: infinite
    -O [ORIGIN]
        Push the splitted input from ORIGIN into ARRAY_NAME, if the option -O is not supplied, the \
           ARRAY_NAME will be erased.
           ORIGIN cannot be greater than the current ARRAY_NAME length
    -s [COUNT]
        Skip the COUNT 'lines', default: 0
        if COUNT is greater than the number of splitted 'lines', nothing will be stored
    -u [FILENAME]
        Read from FILENAME instead of the standard input
    
    "
)]
pub fn mapfile(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    let get_next = |i: &mut usize, msg_err: &str| -> Result<&str, Status> {
        match args.get(*i + 1) {
            Some(r) => {
                *i += 1;
                Ok(r)
            }
            None => return Err(Status::error(msg_err)),
        }
    };

    let mut keep_empty = false;
    let mut discard_nb: u32 = 0;
    let mut delimiter = "\n";
    let mut input = None;
    // TODO better method perhaps
    let mut at_most: u32 = u32::MAX;
    let mut origin: u32 = 0;
    let mut name = "MAPFILE";

    let mut parse_arg = |i: &mut usize| -> Result<(), Status> {
        let arg: &str = args.get(*i).unwrap().as_ref();
        match arg {
            _ if arg.starts_with("--") => match arg {
                "--keep-empty" => keep_empty = true,
                _ => return Err(Status::error(format!("ion: '{}' is not an option", arg))),
            },
            _ if arg.starts_with('-') => match arg.bytes().nth(1) {
                Some(c) => match c {
                    // file option
                    b'u' => {
                        if arg.len() > 2 {
                            return Err(Status::error(format!(
                                "ion: Unexpected '{}' after the -u option",
                                &arg[2..],
                            )));
                        }
                        input = Some(
                            get_next(i, "ion: Expected file path after -u option")?.to_owned(),
                        );
                    }
                    // delimiter option
                    b'd' => {
                        delimiter = match arg.len() {
                            2 => get_next(i, "ion: Expected delimiter")?,
                            _ => &arg[2..],
                        }
                    }
                    // discard option
                    b's' => {
                        discard_nb = match arg.len() {
                            2 => get_next(i, "ion: Expected option (-s)")?,
                            _ => &arg[2..],
                        }
                        .parse::<u32>()
                        .map_err(|_| {
                            Status::error("ion: Param of discard must be a positive integer")
                        })?
                    }
                    // at most option
                    b'n' => {
                        at_most = match arg.len() {
                            2 => get_next(i, "ion: Expected origini after -n param")?,
                            _ => &arg[2..],
                        }
                        .parse::<u32>()
                        .map_err(|_| {
                            Status::error("ion: Param of discard must be a positive integer")
                        })?
                    }
                    // origin option
                    b'O' => {
                        origin = match arg.len() {
                            2 => get_next(i, "ion: Expected origin after -O param")?,
                            _ => &arg[2..],
                        }
                        .parse::<u32>()
                        .map_err(|_| {
                            Status::error("ion: Param of discard must be a positive integer")
                        })?
                    }
                    k => {
                        return Err(Status::error(format!(
                            "ion: mapfile command has not '{}' as option",
                            k as char
                        )))
                    }
                },
                None => return Err(Status::error("ion: Unexpected single '-' as option")),
            },
            s => name = s,
        }
        Ok(())
    };

    let mut set_args = || -> Result<(), Status> {
        let mut i = 1;
        while i < args.len() {
            parse_arg(&mut i)?;
            i += 1;
        }
        Ok(())
    };

    // settings options
    if let Err(status) = set_args() {
        return status;
    }

    let mut inner = |reader: brs::BufReadSplitter<'_, brs::SimpleMatcher>| -> Status {
        let mut lines = BufSplitterIterator::new(reader)
            .into_iter()
            // TODO maybe handle Result here, how ??
            .filter(|v| v.as_ref().is_ok_and(|v| keep_empty || !v.is_empty()))
            .flatten()
            .skip(discard_nb as usize)
            .take(at_most as usize);

        match shell.variables_mut().get_mut(name) {
            // the variable already exists, edit it
            Some(value) => match value {
                Value::Array(data) => {
                    if origin != 0 {
                        if origin as usize > data.len() {
                            return Status::error(format!(
                                "ion: Cannot insert at origin {}, the array length is {}",
                                origin,
                                data.len()
                            ));
                        }
                        // after origin replace each value
                        for d in &mut data[origin as usize..] {
                            if let Some(line) = lines.next() {
                                match d {
                                    Value::Str(s) => {
                                        s.clear();
                                        s.push_str(&line);
                                    }
                                    _ => {
                                        let _ = std::mem::replace::<
                                            Value<std::rc::Rc<types::Function>>,
                                        >(
                                            d, Value::Str(small::String::from(line))
                                        );
                                    }
                                }
                            }
                        }
                        // and push the rest of it if necessary
                        while let Some(line) = lines.next() {
                            data.push(line.into());
                        }
                    } else {
                        data.clear();
                        data.extend(lines.map(|v| v.into()));
                    }
                }
                _ => return Status::error("ion: The current variable is not an array"),
            },
            None => {
                if origin != 0 {
                    return Status::error(format!(
                        "ion: Cannot insert at origin {} if the array is empty",
                        origin
                    ));
                }
                let array = lines.map(|v| v.into()).collect::<Array<_>>();
                shell.variables_mut().set(name, array);
            }
        }

        Status::SUCCESS
    };

    if let Some(filename) = input {
        let mut file = match std::fs::File::open::<&str>(filename.as_ref()) {
            Ok(f) => f,
            Err(err) => {
                return Status::error(format!("ion: Could not open file '{}' ({})", filename, err))
            }
        };
        let reader = brs::BufReadSplitter::new(
            &mut file,
            brs::SimpleMatcher::new(delimiter.as_bytes()),
            brs::Options::default(),
        );
        inner(reader)
    } else {
        // no file provided (-u) than read from stdin
        let mut input: Vec<u8> = Vec::new();
        if atty::is(atty::Stream::Stdin) {
            let mut ctx = Context::new();
            // Prompt needs to be not empty, otherwise a new line is printed instead of void
            while let Ok(line) = ctx.read_line(Prompt::from("\0"), None, &mut EmptyCompleter) {
                input.extend(line.as_bytes());
                input.push(b'\n');
            }
        } else {
            let stdin = std::io::stdin();
            let handle = stdin.lock();
            for line in handle.lines() {
                if let Ok(line) = line {
                    input.extend_from_slice(&line.as_bytes());
                    input.push(b'\n');
                }
            }
        }
        let input_sliced: &mut &[u8] = &mut input.as_slice();
        let reader = brs::BufReadSplitter::new(
            input_sliced,
            brs::SimpleMatcher::new(delimiter.as_bytes()),
            brs::Options::default(),
        );
        inner(reader)
    }
}
