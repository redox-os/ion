
use std::error::Error;
use std::fs;
use std::io::{self, BufWriter};
use smallstring::SmallString;

const MAN_PAGE: &'static str = r#"NAME
    exists - perform tests on files and text

SYNOPSIS
    test [EXPRESSION]

DESCRIPTION
    Tests the expressions given and returns an exit status of 0 if true, else 1.

OPTIONS
    -a ARRAY
        array var is not empty

    -b BINARY
        binary is in PATH

    -d PATH
        path is a directory

    -f PATH
        path is a file

    -s STRING
        string var is not empty

    STRING
        string is not empty

EXAMPLES
    Test if the file exists:
        exists -f FILE && echo "The FILE exists" || echo "The FILE does not exist"

AUTHOR
    Written by Fabian Würfl.
    Heavily based on implementation of the test builtin, which was written by Michael Murph.
"#; /* @MANEND */

pub fn exists(args: &[&str]) -> Result<bool, String> {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    let arguments = &args[1..];
    evaluate_arguments(arguments, &mut buffer)
}

fn evaluate_arguments<W: io::Write>(arguments: &[&str], buffer: &mut W) -> Result<bool, String> {
    match arguments.first() {
        Some(&"--help") => {
            buffer.write_all(MAN_PAGE.as_bytes()).map_err(|x| {
                x.description().to_owned()
            })?;
            buffer.flush().map_err(|x| x.description().to_owned())?;
            Ok(true)
        }
        Some(&s) if s.starts_with("-") => {
            // Access the second character in the flag string: this will be type of the flag.
            // If no flag was given, return `SUCCESS`
            s.chars().nth(1).map_or(Ok(true), |flag| {
                // If no argument was given, return `SUCCESS`
                arguments.get(1).map_or(Ok(true), {
                    |arg|
                    // Match the correct function to the associated flag
                    Ok(match_flag_argument(flag, arg))
                })
            })
        }
        Some(arg) => {
            // If there is no operator, check if the first argument is non-zero
            arguments.get(1).map_or(
                Ok(string_is_nonzero(arg)),
                |operator| {
                    // TODO: STRING check
                    Err("exists: Not implemented.".to_owned())
                },
            )
        }
        None => Ok(false),
    }
}

/// Matches flag arguments to their respective functionaity when the `-` character is detected.
fn match_flag_argument(flag: char, argument: &str) -> bool {
    match flag {
        'a' => array_var_is_not_empty(argument),
        'b' => binary_is_in_path(argument),
        'd' => path_is_directory(argument),
        'f' => path_is_file(argument),
        's' => string_var_is_not_empty(argument),
        _ => false,
    }
}

/// Returns true if the file is a regular file
fn path_is_file(filepath: &str) -> bool {
    fs::metadata(filepath).ok().map_or(false, |metadata| {
        metadata.file_type().is_file()
    })
}

/// Returns true if the file is a directory
fn path_is_directory(filepath: &str) -> bool {
    fs::metadata(filepath).ok().map_or(false, |metadata| {
        metadata.file_type().is_dir()
    })
}

/// Returns true if the binary is found in path (and is executable)
fn binary_is_in_path(filepath: &str) -> bool {
    false
}

/// Returns true if the string is not empty
fn string_is_nonzero(string: &str) -> bool { !string.is_empty() }

/// Returns true if the variable is an array and the array is not empty
fn array_var_is_not_empty(arrayvar: &str) -> bool {
    false
}

/// Returns true if the variable is a string and the array is not empty
fn string_var_is_not_empty(stringvar: &str) -> bool {
    false
}


#[test]
fn test_strings() {
    assert_eq!(string_is_zero("NOT ZERO"), false);
    assert_eq!(string_is_zero(""), true);
    assert_eq!(string_is_nonzero("NOT ZERO"), true);
    assert_eq!(string_is_nonzero(""), false);
}

#[test]
fn test_empty_str() {
    let mut empty = BufWriter::new(io::sink());
    let mut eval = |args: Vec<&str>| evaluate_arguments(&args, &mut empty);
    assert_eq!(eval(vec![""]), Ok(false));
    assert_eq!(eval(vec!["c", "=", ""]), Ok(false));
}

#[test]
fn test_file_exists() {
    assert_eq!(file_exists("testing/empty_file"), true);
    assert_eq!(file_exists("this-does-not-exist"), false);
}

#[test]
fn test_file_is_regular() {
    assert_eq!(file_is_regular("testing/empty_file"), true);
    assert_eq!(file_is_regular("testing"), false);
}

#[test]
fn test_file_is_directory() {
    assert_eq!(file_is_directory("testing"), true);
    assert_eq!(file_is_directory("testing/empty_file"), false);
}
