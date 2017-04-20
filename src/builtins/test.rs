use std::io::{self, Write, BufWriter};
use std::fs;
use std::path::Path;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::time::SystemTime;
use std::error::Error;

const MAN_PAGE: &'static str = /* @MANSTART{test} */ r#"
NAME
    test - perform tests on files and text

SYNOPSIS
    test [EXPRESSION]

DESCRIPTION
    Tests the expressions given and returns an exit status of 0 if true, else 1.

OPTIONS
    -n STRING
        the length of STRING is nonzero

    STRING
        equivalent to -n STRING

    -z STRING
        the length of STRING is zero

    STRING = STRING
        the strings are equivalent

    STRING != STRING
        the strings are not equal

    INTEGER -eq INTEGER
        the integers are equal

    INTEGER -ge INTEGER
        the first INTEGER is greater than or equal to the first INTEGER

    INTEGER -gt INTEGER
        the first INTEGER is greater than the first INTEGER

    INTEGER -le INTEGER
        the first INTEGER is less than or equal to the first INTEGER

    INTEGER -lt INTEGER
        the first INTEGER is less than the first INTEGER

    INTEGER -ne INTEGER
        the first INTEGER is not equal to the first INTEGER

    FILE -ef FILE
        both files have the same device and inode numbers

    FILE -nt FILE
        the first FILE is newer than the second FILE

    FILE -ot FILE
        the first file is older than the second FILE

    -b FILE
        FILE exists and is a block device

    -c FILE
        FILE exists and is a character device

    -d FILE
        FILE exists and is a directory

    -e FILE
        FILE exists

    -f FILE
        FILE exists and is a regular file

    -h FILE
        FILE exists and is a symbolic link (same as -L)

    -L FILE
        FILE exists and is a symbolic link (same as -h)

    -r FILE
        FILE exists and read permission is granted

    -s FILE
        FILE exists and has a file size greater than zero

    -S FILE
        FILE exists and is a socket

    -w FILE
        FILE exists and write permission is granted

    -x FILE
        FILE exists and execute (or search) permission is granted

EXAMPLES
    Test if the file exists:
        test -e FILE && echo "The FILE exists" || echo "The FILE does not exist"

    Test if the file exists and is a regular file, and if so, write to it:
        test -f FILE && echo "Hello, FILE" >> FILE || echo "Cannot write to a directory"

    Test if 10 is greater than 5:
        test 10 -gt 5 && echo "10 is greater than 5" || echo "10 is not greater than 5"

    Test if the user is running a 64-bit OS (POSIX environment only):
        test $(getconf LONG_BIT) = 64 && echo "64-bit OS" || echo "32-bit OS"

AUTHOR
    Written by Michael Murphy.
"#; /* @MANEND */

pub fn test(args: &[String]) -> Result<bool, String> {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());
    
    let arguments = &args[1..];
    evaluate_arguments(arguments, &mut buffer)
}

fn evaluate_arguments(arguments: &[String], buffer: &mut BufWriter<io::StdoutLock>) -> Result<bool, String> {
    if let Some(arg) = arguments.first() {
        if arg.as_str() == "--help" {
            match buffer.write_all(MAN_PAGE.as_bytes()) {
                Err(why) => return Err(String::from(why.description())),
                _ => {}
            }

            match buffer.flush() {
                Err(why) => return Err(String::from(why.description())),
                _ => {}
            }

            return Ok(true);
        }
        let mut characters = arg.chars().take(2);
        return match characters.next().unwrap() {
            '-' => {
                // If no flag was given, return `SUCCESS`
                characters.next().map_or(Ok(true), |flag| {
                    // If no argument was given, return `SUCCESS`
                    arguments.get(1).map_or(Ok(true), |argument| {
                        // match the correct function to the associated flag
                        Ok(match_flag_argument(flag, argument.as_str()))
                    })
                })
            },
            _   => {
                // If there is no operator, check if the first argument is non-zero
                arguments.get(1).map_or(Ok(string_is_nonzero(&arg)), |operator| {
                    // If there is no right hand argument, a condition was expected
                    match arguments.get(2) {
                        Some(right_arg) => evaluate_expression(arg.as_str(), operator.as_str(), right_arg.as_str()),
                        None => {
                            Err(String::from("parse error: condition expected"))
                        }
                    }
                })
            },
        };
    } else {
        return Ok(false);
    }
}

fn evaluate_expression(first: &str, operator: &str, second: &str) -> Result<bool, String> {
    match operator {
        "=" | "==" => Ok(evaluate_bool(first == second)),
        "!="       => Ok(evaluate_bool(first != second)),
        "-ef"      => Ok(files_have_same_device_and_inode_numbers(first, second)),
        "-nt"      => Ok(file_is_newer_than(first, second)),
        "-ot"      => Ok(file_is_newer_than(second, first)),
        _          => {
            let (left, right) = parse_integers(first, second)?;
            match operator {
                "-eq" => Ok(evaluate_bool(left == right)),
                "-ge" => Ok(evaluate_bool(left >= right)),
                "-gt" => Ok(evaluate_bool(left > right)),
                "-le" => Ok(evaluate_bool(left <= right)),
                "-lt" => Ok(evaluate_bool(left < right)),
                "-ne" => Ok(evaluate_bool(left != right)),
                _     => {
                    Err(format!("unknowne condition: {:?}", operator))
                }
            }
        }
    }

}

/// Exits SUCCESS if both files have the same device and inode numbers
fn files_have_same_device_and_inode_numbers(first: &str, second: &str) -> bool {
    // Obtain the device and inode of the first file or return FAILED
    get_dev_and_inode(first).map_or(false, |left| {
        // Obtain the device and inode of the second file or return FAILED
        get_dev_and_inode(second).map_or(false, |right| {
            // Compare the device and inodes of the first and second files
            evaluate_bool(left == right)
        })
    })
}

/// Obtains the device and inode numbers of the file specified
fn get_dev_and_inode(filename: &str) -> Option<(u64, u64)> {
    fs::metadata(filename).map(|file| (file.dev(), file.ino())).ok()
}

/// Exits SUCCESS if the first file is newer than the second file.
fn file_is_newer_than(first: &str, second: &str) -> bool {
    // Obtain the modified file time of the first file or return FAILED
    get_modified_file_time(first).map_or(false, |left| {
        // Obtain the modified file time of the second file or return FAILED
        get_modified_file_time(second).map_or(false, |right| {
            // If the first file is newer than the right file, return SUCCESS
            evaluate_bool(left > right)
        })
    })
}

/// Obtain the time the file was last modified as a `SystemTime` type.
fn get_modified_file_time(filename: &str) -> Option<SystemTime> {
    fs::metadata(filename).ok().and_then(|file| file.modified().ok())
}

/// Attempt to parse a &str as a usize.
fn parse_integers(left: &str, right: &str) -> Result<(Option<usize>, Option<usize>), String> {
    let parse_integer = |input: &str| -> Result<Option<usize>, String> {
        match input.parse::<usize>().map_err(|_| {
            format!("integer expression expected: {:?}", input)
        }) {
            Err(why) => Err(String::from(why)),
            Ok(res) => Ok(Some(res)),
        }
    };

    match (parse_integer(left), parse_integer(right)) {
        (Err(left), Err(_)) => Err(left),
        (Ok(_), Err(right)) => Err(right),
        (Err(left), Ok(_)) => Err(left),
        (Ok(left), Ok(right)) => Ok((left, right)),
    }
}

/// Matches flag arguments to their respective functionaity when the `-` character is detected.
fn match_flag_argument(flag: char, argument: &str) -> bool {
    // TODO: Implement missing flags
    match flag {
        'b' => file_is_block_device(argument),
        'c' => file_is_character_device(argument),
        'd' => file_is_directory(argument),
        'e' => file_exists(argument),
        'f' => file_is_regular(argument),
        //'g' => file_is_set_group_id(argument),
        //'G' => file_is_owned_by_effective_group_id(argument),
        'h' | 'L' => file_is_symlink(argument),
        //'k' => file_has_sticky_bit(argument),
        //'O' => file_is_owned_by_effective_user_id(argument),
        //'p' => file_is_named_pipe(argument),
        'r' => file_has_read_permission(argument),
        's' => file_size_is_greater_than_zero(argument),
        'S' => file_is_socket(argument),
        //'t' => file_descriptor_is_opened_on_a_terminal(argument),
        'w' => file_has_write_permission(argument),
        'x' => file_has_execute_permission(argument),
        'n' => string_is_nonzero(argument),
        'z' => string_is_zero(argument),
        _ => true,
    }
}

/// Exits SUCCESS if the file size is greather than zero.
fn file_size_is_greater_than_zero(filepath: &str) -> bool {
    fs::metadata(filepath).ok().map_or(false, |metadata| evaluate_bool(metadata.len() > 0))
}

/// Exits SUCCESS if the file has read permissions. This function is rather low level because
/// Rust currently does not have a higher level abstraction for obtaining non-standard file modes.
/// To extract the permissions from the mode, the bitwise AND operator will be used and compared
/// with the respective read bits.
fn file_has_read_permission(filepath: &str) -> bool {
    const USER_BIT:  u32 = 0b100000000;
    const GROUP_BIT: u32 = 0b100000;
    const GUEST_BIT: u32 = 0b100;

    // Collect the mode of permissions for the file
    fs::metadata(filepath).map(|metadata| metadata.permissions().mode()).ok()
        // If the mode is equal to any of the above, return `SUCCESS`
        .map_or(false, |mode| {
            if mode & USER_BIT == USER_BIT || mode & GROUP_BIT == GROUP_BIT ||
                mode & GUEST_BIT == GUEST_BIT { true } else { false }
        })
}

/// Exits SUCCESS if the file has write permissions. This function is rather low level because
/// Rust currently does not have a higher level abstraction for obtaining non-standard file modes.
/// To extract the permissions from the mode, the bitwise AND operator will be used and compared
/// with the respective write bits.
fn file_has_write_permission(filepath: &str) -> bool {
    const USER_BIT:  u32 = 0b10000000;
    const GROUP_BIT: u32 = 0b10000;
    const GUEST_BIT: u32 = 0b10;

    // Collect the mode of permissions for the file
    fs::metadata(filepath).map(|metadata| metadata.permissions().mode()).ok()
        // If the mode is equal to any of the above, return `SUCCESS`
        .map_or(false, |mode| {
            if mode & USER_BIT == USER_BIT || mode & GROUP_BIT == GROUP_BIT ||
                mode & GUEST_BIT == GUEST_BIT { true } else { false }
        })
}

/// Exits SUCCESS if the file has execute permissions. This function is rather low level because
/// Rust currently does not have a higher level abstraction for obtaining non-standard file modes.
/// To extract the permissions from the mode, the bitwise AND operator will be used and compared
/// with the respective execute bits.
fn file_has_execute_permission(filepath: &str) -> bool {
    const USER_BIT:  u32 = 0b1000000;
    const GROUP_BIT: u32 = 0b1000;
    const GUEST_BIT: u32 = 0b1;

    // Collect the mode of permissions for the file
    fs::metadata(filepath).map(|metadata| metadata.permissions().mode()).ok()
        // If the mode is equal to any of the above, return `SUCCESS`
        .map_or(false, |mode| {
            if mode & USER_BIT == USER_BIT || mode & GROUP_BIT == GROUP_BIT ||
                mode & GUEST_BIT == GUEST_BIT { true } else { false }
        })
}

/// Exits SUCCESS if the file argument is a socket
fn file_is_socket(filepath: &str) -> bool {
    fs::metadata(filepath).ok()
        .map_or(false, |metadata| evaluate_bool(metadata.file_type().is_socket()))
}

/// Exits SUCCESS if the file argument is a block device
fn file_is_block_device(filepath: &str) -> bool {
    fs::metadata(filepath).ok()
        .map_or(false, |metadata| evaluate_bool(metadata.file_type().is_block_device()))
}

/// Exits SUCCESS if the file argument is a character device
fn file_is_character_device(filepath: &str) -> bool {
    fs::metadata(filepath).ok()
        .map_or(false, |metadata| evaluate_bool(metadata.file_type().is_char_device()))
}

/// Exits SUCCESS if the file exists
fn file_exists(filepath: &str) -> bool {
    evaluate_bool(Path::new(filepath).exists())
}

/// Exits SUCCESS if the file is a regular file
fn file_is_regular(filepath: &str) -> bool {
    fs::metadata(filepath).ok()
        .map_or(false, |metadata| evaluate_bool(metadata.file_type().is_file()))
}

/// Exits SUCCESS if the file is a directory
fn file_is_directory(filepath: &str) -> bool {
    fs::metadata(filepath).ok()
        .map_or(false, |metadata| evaluate_bool(metadata.file_type().is_dir()))
}

/// Exits SUCCESS if the file is a symbolic link
fn file_is_symlink(filepath: &str) -> bool {
    fs::symlink_metadata(filepath).ok()
        .map_or(false, |metadata| evaluate_bool(metadata.file_type().is_symlink()))
}

/// Exits SUCCESS if the string is not empty
fn string_is_nonzero(string: &str) -> bool {
    evaluate_bool(!string.is_empty())
}

/// Exits SUCCESS if the string is empty
fn string_is_zero(string: &str) -> bool {
    evaluate_bool(string.is_empty())
}

/// Convert a boolean to it's respective exit code.
fn evaluate_bool(input_is_true: bool) -> bool { if input_is_true { true } else { false } }

#[test]
fn test_strings() {
    assert_eq!(string_is_zero("NOT ZERO"), false);
    assert_eq!(string_is_zero(""), true);
    assert_eq!(string_is_nonzero("NOT ZERO"), true);
    assert_eq!(string_is_nonzero(""), false);
}

#[test]
fn test_integers_arguments() {
    let stdout = io::stdout();
    let mut buffer = BufWriter::new(stdout.lock());

    // Equal To
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-eq"), String::from("10")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-eq"), String::from("5")],
        &mut buffer), Ok(false));

    // Greater Than or Equal To
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-ge"), String::from("10")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-ge"), String::from("5")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("5"), String::from("-ge"), String::from("10")],
        &mut buffer), Ok(false));

    // Less Than or Equal To
    assert_eq!(evaluate_arguments(&[String::from("5"), String::from("-le"), String::from("5")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("5"), String::from("-le"), String::from("10")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-le"), String::from("5")],
        &mut buffer), Ok(false));

    // Less Than
    assert_eq!(evaluate_arguments(&[String::from("5"), String::from("-lt"), String::from("10")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-lt"), String::from("5")],
        &mut buffer), Ok(false));

    // Greater Than
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-gt"), String::from("5")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("5"), String::from("-gt"), String::from("10")],
        &mut buffer), Ok(false));

    // Not Equal To
    assert_eq!(evaluate_arguments(&[String::from("10"), String::from("-ne"), String::from("5")],
        &mut buffer), Ok(true));
    assert_eq!(evaluate_arguments(&[String::from("5"), String::from("-ne"), String::from("5")],
        &mut buffer), Ok(false));
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

#[test]
fn test_file_is_symlink() {
    assert_eq!(file_is_symlink("testing/symlink"), true);
    assert_eq!(file_is_symlink("testing/empty_file"), false);
}

#[test]
fn test_file_has_execute_permission() {
    assert_eq!(file_has_execute_permission("testing/executable_file"), true);
    assert_eq!(file_has_execute_permission("testing/empty_file"), false);
}

#[test]
fn test_file_size_is_greater_than_zero() {
    assert_eq!(file_size_is_greater_than_zero("testing/file_with_text"), true);
    assert_eq!(file_size_is_greater_than_zero("testing/empty_file"), false);
}
