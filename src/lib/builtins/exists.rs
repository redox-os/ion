#[cfg(test)]
use smallstring::SmallString;
#[cfg(test)]
use smallvec::SmallVec;
use std::{fs, os::unix::fs::PermissionsExt};

#[cfg(test)]
use shell;
#[cfg(test)]
use shell::flow_control::{Function, Statement};
use shell::Shell;

pub(crate) fn exists(args: &[String], shell: &Shell) -> Result<bool, String> {
    let arguments = &args[1..];
    evaluate_arguments(arguments, shell)
}

fn evaluate_arguments(arguments: &[String], shell: &Shell) -> Result<bool, String> {
    match arguments.first() {
        Some(ref s) if s.starts_with("--") => {
            let (_, option) = s.split_at(2);
            // If no argument was given, return `SUCCESS`, as this means a string starting
            // with a dash was given
            arguments.get(1).map_or(Ok(true), {
                |arg|
                // Match the correct function to the associated flag
                Ok(match_option_argument(option, arg, shell))
            })
        }
        Some(ref s) if s.starts_with('-') => {
            // Access the second character in the flag string: this will be type of the
            // flag. If no flag was given, return `SUCCESS`, as this means a
            // string with value "-" was checked.
            s.chars().nth(1).map_or(Ok(true), |flag| {
                // If no argument was given, return `SUCCESS`, as this means a string starting
                // with a dash was given
                arguments.get(1).map_or(Ok(true), {
                    |arg|
                    // Match the correct function to the associated flag
                    Ok(match_flag_argument(flag, arg, shell))
                })
            })
        }
        Some(string) => Ok(string_is_nonzero(string)),
        None => Ok(false),
    }
}

/// Matches flag arguments to their respective functionaity when the `-`
/// character is detected.
fn match_flag_argument(flag: char, argument: &str, shell: &Shell) -> bool {
    match flag {
        'a' => array_var_is_not_empty(argument, shell),
        'b' => binary_is_in_path(argument, shell),
        'd' => path_is_directory(argument),
        'f' => path_is_file(argument),
        's' => string_var_is_not_empty(argument, shell),
        _ => false,
    }
}

// Matches option arguments to their respective functionality
fn match_option_argument(option: &str, argument: &str, shell: &Shell) -> bool {
    match option {
        "fn" => function_is_defined(argument, &shell),
        _ => false,
    }
}

/// Returns true if the file is a regular file
fn path_is_file(filepath: &str) -> bool {
    fs::metadata(filepath)
        .ok()
        .map_or(false, |metadata| metadata.file_type().is_file())
}

/// Returns true if the file is a directory
fn path_is_directory(filepath: &str) -> bool {
    fs::metadata(filepath)
        .ok()
        .map_or(false, |metadata| metadata.file_type().is_dir())
}

/// Returns true if the binary is found in path (and is executable)
fn binary_is_in_path(binaryname: &str, shell: &Shell) -> bool {
    // TODO: Maybe this function should reflect the logic for spawning new processes
    // TODO: Right now they use an entirely different logic which means that it
    // *might* be possible TODO: that `exists` reports a binary to be in the
    // path, while the shell cannot find it or TODO: vice-versa
    if let Some(path) = shell.get_var("PATH") {
        for dir in path.split(':') {
            let fname = format!("{}/{}", dir, binaryname);
            if let Ok(metadata) = fs::metadata(&fname) {
                if metadata.is_file() && file_has_execute_permission(&fname) {
                    return true;
                }
            }
        }
    };

    false
}

/// Returns true if the file has execute permissions. This function is rather low level because
/// Rust currently does not have a higher level abstraction for obtaining non-standard file modes.
/// To extract the permissions from the mode, the bitwise AND operator will be used and compared
/// with the respective execute bits.
/// Note: This function is 1:1 the same as src/builtins/test.rs:file_has_execute_permission
/// If you change the following function, please also update the one in src/builtins/test.rs
fn file_has_execute_permission(filepath: &str) -> bool {
    const USER: u32 = 0b100_0000;
    const GROUP: u32 = 0b1000;
    const GUEST: u32 = 0b1;

    // Collect the mode of permissions for the file
    fs::metadata(filepath).map(|metadata| metadata.permissions().mode()).ok()
        // If the mode is equal to any of the above, return `SUCCESS`
        .map_or(false, |mode| mode & (USER + GROUP + GUEST) != 0)
}

/// Returns true if the string is not empty
fn string_is_nonzero(string: &str) -> bool { !string.is_empty() }

/// Returns true if the variable is an array and the array is not empty
fn array_var_is_not_empty(arrayvar: &str, shell: &Shell) -> bool {
    match shell.variables.get_array(arrayvar) {
        Some(array) => !array.is_empty(),
        None => false,
    }
}

/// Returns true if the variable is a string and the string is not empty
fn string_var_is_not_empty(stringvar: &str, shell: &Shell) -> bool {
    match shell.get_var(stringvar) {
        Some(string) => !string.is_empty(),
        None => false,
    }
}

/// Returns true if a function with the given name is defined
fn function_is_defined(function: &str, shell: &Shell) -> bool {
    match shell.variables.get_function(function) {
        Some(_) => true,
        None => false,
    }
}

#[test]
fn test_evaluate_arguments() {
    use parser::assignments::{KeyBuf, Primitive};
    let mut shell = shell::ShellBuilder::new().as_library();

    // assert_eq!(evaluate_arguments(&[], &mut sink, &shell), Ok(false));
    // no parameters
    assert_eq!(evaluate_arguments(&[], &shell), Ok(false));
    // multiple arguments
    // ignores all but the first argument
    assert_eq!(evaluate_arguments(&["foo".to_owned(), "bar".to_owned()], &shell), Ok(true));

    // check `exists STRING`
    assert_eq!(evaluate_arguments(&["".to_owned()], &shell), Ok(false));
    assert_eq!(evaluate_arguments(&["string".to_owned()], &shell), Ok(true));
    assert_eq!(evaluate_arguments(&["string with space".to_owned()], &shell), Ok(true));
    assert_eq!(evaluate_arguments(&["-startswithdash".to_owned()], &shell), Ok(true));

    // check `exists -a`
    // no argument means we treat it as a string
    assert_eq!(evaluate_arguments(&["-a".to_owned()], &shell), Ok(true));
    shell
        .variables
        .set_array("emptyarray", SmallVec::from_vec(Vec::new()));
    assert_eq!(evaluate_arguments(&["-a".to_owned(), "emptyarray".to_owned()], &shell), Ok(false));
    let mut vec = Vec::new();
    vec.push("element".to_owned());
    shell.variables.set_array("array", SmallVec::from_vec(vec));
    assert_eq!(evaluate_arguments(&["-a".to_owned(), "array".to_owned()], &shell), Ok(true));
    shell.variables.unset_array("array");
    assert_eq!(evaluate_arguments(&["-a".to_owned(), "array".to_owned()], &shell), Ok(false));

    // check `exists -b`
    // TODO: see test_binary_is_in_path()
    // no argument means we treat it as a string
    assert_eq!(evaluate_arguments(&["-b".to_owned()], &shell), Ok(true));
    let oldpath = shell.get_var("PATH").unwrap_or("/usr/bin".to_owned());
    shell.set_var("PATH", "testing/");

    assert_eq!(
        evaluate_arguments(&["-b".to_owned(), "executable_file".to_owned()], &shell),
        Ok(true)
    );
    assert_eq!(evaluate_arguments(&["-b".to_owned(), "empty_file".to_owned()], &shell), Ok(false));
    assert_eq!(
        evaluate_arguments(&["-b".to_owned(), "file_does_not_exist".to_owned()], &shell),
        Ok(false)
    );

    // restore original PATH. Not necessary for the currently defined test cases
    // but this might change in the future? Better safe than sorry!
    shell.set_var("PATH", &oldpath);

    // check `exists -d`
    // no argument means we treat it as a string
    assert_eq!(evaluate_arguments(&["-d".to_owned()], &shell), Ok(true));
    assert_eq!(evaluate_arguments(&["-d".to_owned(), "testing/".to_owned()], &shell), Ok(true));
    assert_eq!(
        evaluate_arguments(&["-d".to_owned(), "testing/empty_file".to_owned()], &shell),
        Ok(false)
    );
    assert_eq!(
        evaluate_arguments(&["-d".to_owned(), "does/not/exist/".to_owned()], &shell),
        Ok(false)
    );

    // check `exists -f`
    // no argument means we treat it as a string
    assert_eq!(evaluate_arguments(&["-f".to_owned()], &shell), Ok(true));
    assert_eq!(evaluate_arguments(&["-f".to_owned(), "testing/".to_owned()], &shell), Ok(false));
    assert_eq!(
        evaluate_arguments(&["-f".to_owned(), "testing/empty_file".to_owned()], &shell),
        Ok(true)
    );
    assert_eq!(
        evaluate_arguments(&["-f".to_owned(), "does-not-exist".to_owned()], &shell),
        Ok(false)
    );

    // check `exists -s`
    // no argument means we treat it as a string
    assert_eq!(evaluate_arguments(&["-s".to_owned()], &shell), Ok(true));
    shell.set_var("emptyvar", "");
    assert_eq!(evaluate_arguments(&["-s".to_owned(), "emptyvar".to_owned()], &shell), Ok(false));
    shell.set_var("testvar", "foobar");
    assert_eq!(evaluate_arguments(&["-s".to_owned(), "testvar".to_owned()], &shell), Ok(true));
    shell.variables.unset_var("testvar");
    assert_eq!(evaluate_arguments(&["-s".to_owned(), "testvar".to_owned()], &shell), Ok(false));
    // also check that it doesn't trigger on arrays
    let mut vec = Vec::new();
    vec.push("element".to_owned());
    shell.variables.unset_var("array");
    shell.variables.set_array("array", SmallVec::from_vec(vec));
    assert_eq!(evaluate_arguments(&["-s".to_owned(), "array".to_owned()], &shell), Ok(false));

    // check `exists --fn`
    let name_str = "test_function";
    let name = SmallString::from_str(name_str);
    let mut args = Vec::new();
    args.push(KeyBuf {
        name: "testy".into(),
        kind: Primitive::Any,
    });
    let mut statements = Vec::new();
    statements.push(Statement::End);
    let description = "description".to_owned();

    shell.variables.insert_function(
        name.clone(),
        Function::new(Some(description), name, args, statements),
    );

    assert_eq!(evaluate_arguments(&["--fn".to_owned(), name_str.to_owned()], &shell), Ok(true));
    shell.variables.remove_function(name_str);
    assert_eq!(evaluate_arguments(&["--fn".to_owned(), name_str.to_owned()], &shell), Ok(false));

    // check invalid flags / parameters (should all be treated as strings and
    // therefore succeed)
    assert_eq!(evaluate_arguments(&["--foo".to_owned()], &shell), Ok(true));
    assert_eq!(evaluate_arguments(&["-x".to_owned()], &shell), Ok(true));
}

#[test]
fn test_match_flag_argument() {
    let shell = shell::ShellBuilder::new().as_library();

    // we don't really care about the passed values, as long as both sited return
    // the same value
    assert_eq!(
        match_flag_argument('a', "ARRAY", &shell),
        array_var_is_not_empty("ARRAY", &shell)
    );
    assert_eq!(
        match_flag_argument('b', "binary", &shell),
        binary_is_in_path("binary", &shell)
    );
    assert_eq!(
        match_flag_argument('d', "path", &shell),
        path_is_directory("path")
    );
    assert_eq!(
        match_flag_argument('f', "file", &shell),
        path_is_file("file")
    );
    assert_eq!(
        match_flag_argument('s', "STR", &shell),
        string_var_is_not_empty("STR", &shell)
    );

    // Any flag which is not implemented
    assert_eq!(match_flag_argument('x', "ARG", &shell), false);
}

#[test]
fn test_match_option_argument() {
    let shell = shell::ShellBuilder::new().as_library();

    // we don't really care about the passed values, as long as both sited return
    // the same value
    assert_eq!(
        match_option_argument("fn", "FUN", &shell),
        array_var_is_not_empty("FUN", &shell)
    );

    // Any option which is not implemented
    assert_eq!(match_option_argument("foo", "ARG", &shell), false);
}

#[test]
fn test_path_is_file() {
    assert_eq!(path_is_file("testing/empty_file"), true);
    assert_eq!(path_is_file("this-does-not-exist"), false);
}

#[test]
fn test_path_is_directory() {
    assert_eq!(path_is_directory("testing"), true);
    assert_eq!(path_is_directory("testing/empty_file"), false);
}

#[test]
fn test_binary_is_in_path() {
    let mut shell = shell::ShellBuilder::new().as_library();

    // TODO: We should probably also test with more complex PATH-variables:
    // TODO: multiple/:directories/
    // TODO: PATH containing directories which do not exist
    // TODO: PATH containing directories without read permission (for user)
    // TODO: PATH containing directories without execute ("enter") permission (for
    // user) TODO: empty PATH?
    shell.set_var("PATH", "testing/");

    assert_eq!(binary_is_in_path("executable_file", &shell), true);
    assert_eq!(binary_is_in_path("empty_file", &shell), false);
    assert_eq!(binary_is_in_path("file_does_not_exist", &shell), false);
}

#[test]
fn test_file_has_execute_permission() {
    assert_eq!(file_has_execute_permission("testing/executable_file"), true);
    assert_eq!(file_has_execute_permission("testing"), true);
    assert_eq!(file_has_execute_permission("testing/empty_file"), false);
    assert_eq!(file_has_execute_permission("this-does-not-exist"), false);
}

#[test]
fn test_string_is_nonzero() {
    assert_eq!(string_is_nonzero("NOT ZERO"), true);
    assert_eq!(string_is_nonzero(""), false);
}

#[test]
fn test_array_var_is_not_empty() {
    let mut shell = shell::ShellBuilder::new().as_library();

    shell
        .variables
        .set_array("EMPTY_ARRAY", SmallVec::from_vec(Vec::new()));
    assert_eq!(array_var_is_not_empty("EMPTY_ARRAY", &shell), false);

    let mut not_empty_vec = Vec::new();
    not_empty_vec.push("array not empty".to_owned());
    shell
        .variables
        .set_array("NOT_EMPTY_ARRAY", SmallVec::from_vec(not_empty_vec));
    assert_eq!(array_var_is_not_empty("NOT_EMPTY_ARRAY", &shell), true);

    // test for array which does not even exist
    shell.variables.unset_array("NOT_EMPTY_ARRAY");
    assert_eq!(array_var_is_not_empty("NOT_EMPTY_ARRAY", &shell), false);

    // array_var_is_not_empty should NOT match for non-array variables with the
    // same name
    shell.set_var("VARIABLE", "notempty-variable");
    assert_eq!(array_var_is_not_empty("VARIABLE", &shell), false);
}

#[test]
fn test_string_var_is_not_empty() {
    let mut shell = shell::ShellBuilder::new().as_library();

    shell.set_var("EMPTY", "");
    assert_eq!(string_var_is_not_empty("EMPTY", &shell), false);

    shell.set_var("NOT_EMPTY", "notempty");
    assert_eq!(string_var_is_not_empty("NOT_EMPTY", &shell), true);

    // string_var_is_not_empty should NOT match for arrays with the same name
    let mut vec = Vec::new();
    vec.push("not-empty".to_owned());
    shell
        .variables
        .set_array("ARRAY_NOT_EMPTY", SmallVec::from_vec(vec));
    assert_eq!(string_var_is_not_empty("ARRAY_NOT_EMPTY", &shell), false);

    // test for a variable which does not even exist
    shell.variables.unset_var("NOT_EMPTY");
    assert_eq!(string_var_is_not_empty("NOT_EMPTY", &shell), false);
}

#[test]
fn test_function_is_defined() {
    use parser::assignments::{KeyBuf, Primitive};
    let mut shell = shell::ShellBuilder::new().as_library();

    // create a simple dummy function
    let name_str = "test_function";
    let name = SmallString::from_str(name_str);
    let mut args = Vec::new();
    args.push(KeyBuf {
        name: "testy".into(),
        kind: Primitive::Any,
    });
    let mut statements = Vec::new();
    statements.push(Statement::End);
    let description = "description".to_owned();

    shell.variables.insert_function(
        name.clone(),
        Function::new(Some(description), name, args, statements),
    );

    assert_eq!(function_is_defined(name_str, &shell), true);
    shell.variables.remove_function(name_str);
    assert_eq!(function_is_defined(name_str, &shell), false);
}
