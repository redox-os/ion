use std::{fs, os::unix::fs::PermissionsExt};

use super::Status;
use crate as ion_shell;
use crate::{
    shell::{Shell, Value},
    types,
};
use builtins_proc::builtin;

#[builtin(
    desc = "check whether items exist",
    man = "
SYNOPSIS
    exists [EXPRESSION]

DESCRIPTION
    Checks whether the given item exists and returns an exit status of 0 if it does, else 1.

OPTIONS
    -a ARRAY
        array var is not empty

    -b BINARY
        binary is in PATH

    -d PATH
        path is a directory
        This is the same as test -d

    -f PATH
        path is a file
        This is the same as test -f

    --fn FUNCTION
        function is defined

    -s STRING
        string var is not empty

    STRING
        string is not empty
        This is the same as test -n

EXAMPLES
    Test if the file exists:
        exists -f FILE && echo 'The FILE exists' || echo 'The FILE does not exist'

    Test if some-command exists in the path and is executable:
        exists -b some-command && echo 'some-command exists' || echo 'some-command does not exist'

    Test if variable exists AND is not empty
        exists -s myVar && echo \"myVar exists: $myVar\" || echo 'myVar does not exist or is empty'
        NOTE: Don't use the '$' sigil, but only the name of the variable to check

    Test if array exists and is not empty
        exists -a myArr && echo \"myArr exists: @myArr\" || echo 'myArr does not exist or is empty'
        NOTE: Don't use the '@' sigil, but only the name of the array to check

    Test if a function named 'myFunc' exists
        exists --fn myFunc && myFunc || echo 'No function with name myFunc found'

AUTHOR
    Written by Fabian W\u{00FC}rfl.
    Heavily based on implementation of the test builtin, which was written by Michael Murphy."
)]
pub fn exists(args: &[types::Str], shell: &mut Shell<'_>) -> Status {
    match args.get(1) {
        Some(s) if s.starts_with("--") => {
            let (_, option) = s.split_at(2);
            // If no argument was given, return `SUCCESS`, as this means a string starting
            // with a dash was given
            args.get(2).map_or(true, {
                |arg|
                // Match the correct function to the associated flag
                match_option_argument(option, arg, shell)
            })
        }
        Some(s) if s.starts_with('-') => {
            // Access the second character in the flag string: this will be type of the
            // flag. If no flag was given, return `SUCCESS`, as this means a
            // string with value "-" was checked.
            s.chars().nth(1).map_or(true, |flag| {
                // If no argument was given, return `SUCCESS`, as this means a string starting
                // with a dash was given
                args.get(2).map_or(true, {
                    |arg|
                    // Match the correct function to the associated flag
                    match_flag_argument(flag, arg, shell)
                })
            })
        }
        Some(string) => !string.is_empty(),
        None => false,
    }
    .into()
}

/// Matches flag arguments to their respective functionaity when the `-`
/// character is detected.
fn match_flag_argument(flag: char, argument: &str, shell: &Shell<'_>) -> bool {
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
fn match_option_argument(option: &str, argument: &str, shell: &Shell<'_>) -> bool {
    match option {
        "fn" => function_is_defined(argument, shell),
        _ => false,
    }
}

/// Returns true if the file is a regular file
fn path_is_file(filepath: &str) -> bool {
    fs::metadata(filepath).ok().map_or(false, |metadata| metadata.file_type().is_file())
}

/// Returns true if the file is a directory
fn path_is_directory(filepath: &str) -> bool {
    fs::metadata(filepath).ok().map_or(false, |metadata| metadata.file_type().is_dir())
}

/// Returns true if the binary is found in path (and is executable)
fn binary_is_in_path(binaryname: &str, shell: &Shell<'_>) -> bool {
    // TODO: Maybe this function should reflect the logic for spawning new processes
    // TODO: Right now they use an entirely different logic which means that it
    // *might* be possible that `exists` reports a binary to be in the
    // path, while the shell cannot find it or TODO: vice-versa
    if let Ok(path) = shell.variables().get_str("PATH") {
        for fname in path.split(':').map(|dir| format!("{}/{}", dir, binaryname)) {
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
/// Note: This function is 1:1 the same as `src/builtins/test.rs:file_has_execute_permission`
/// If you change the following function, please also update the one in src/builtins/test.rs
fn file_has_execute_permission(filepath: &str) -> bool {
    const USER: u32 = 0b100_0000;
    const GROUP: u32 = 0b1000;
    const GUEST: u32 = 0b1;

    // Collect the mode of permissions for the file
    fs::metadata(filepath)
        .map(|metadata| metadata.permissions().mode())
        .ok()
        // If the mode is equal to any of the above, return `SUCCESS`
        .map_or(false, |mode| mode & (USER + GROUP + GUEST) != 0)
}

/// Returns true if the variable is an array and the array is not empty
fn array_var_is_not_empty(arrayvar: &str, shell: &Shell<'_>) -> bool {
    match shell.variables().get(arrayvar) {
        Some(Value::Array(array)) => !array.is_empty(),
        _ => false,
    }
}

/// Returns true if the variable is a string and the string is not empty
fn string_var_is_not_empty(stringvar: &str, shell: &Shell<'_>) -> bool {
    match shell.variables().get_str(stringvar) {
        Ok(string) => !string.is_empty(),
        Err(_) => false,
    }
}

/// Returns true if a function with the given name is defined
fn function_is_defined(function: &str, shell: &Shell<'_>) -> bool {
    if let Some(Value::Function(_)) = shell.variables().get(function) {
        true
    } else {
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        flow_control::Function,
        parser::lexers::assignments::{KeyBuf, Primitive},
        shell::flow_control::Statement,
        types,
    };

    #[test]
    fn test_evaluate_arguments() {
        let mut shell = Shell::default();

        // assert_eq!(exists(&["ion".into(), ], &mut sink, &shell), Ok(false));
        // no parameters
        assert!(builtin_exists(&["ion".into()], &mut shell).is_failure());
        // multiple arguments
        // ignores all but the first argument
        assert!(
            builtin_exists(&["ion".into(), "foo".into(), "bar".into()], &mut shell).is_success()
        );

        // check `exists STRING`
        assert!(builtin_exists(&["ion".into(), "".into()], &mut shell).is_failure());
        assert!(builtin_exists(&["ion".into(), "string".into()], &mut shell).is_success());
        assert!(
            builtin_exists(&["ion".into(), "string with space".into()], &mut shell).is_success()
        );
        assert!(builtin_exists(&["ion".into(), "-startswithdash".into()], &mut shell).is_success());

        // check `exists -a`
        // no argument means we treat it as a string
        assert!(builtin_exists(&["ion".into(), "-a".into()], &mut shell).is_success());
        shell.variables_mut().set("emptyarray", types::Array::new());
        assert!(builtin_exists(&["ion".into(), "-a".into(), "emptyarray".into()], &mut shell)
            .is_failure());
        let mut array = types::Array::new();
        array.push("element".into());
        shell.variables_mut().set("array", array);
        assert!(
            builtin_exists(&["ion".into(), "-a".into(), "array".into()], &mut shell).is_success()
        );
        shell.variables_mut().remove("array");
        assert!(
            builtin_exists(&["ion".into(), "-a".into(), "array".into()], &mut shell).is_failure()
        );

        // check `exists -b`
        // TODO: see test_binary_is_in_path()
        // no argument means we treat it as a string
        assert!(builtin_exists(&["ion".into(), "-b".into()], &mut shell).is_success());
        let oldpath = shell.variables().get_str("PATH").unwrap_or_else(|_| "/usr/bin".into());
        shell.variables_mut().set("PATH", "testing/");

        assert!(builtin_exists(&["ion".into(), "-b".into(), "executable_file".into()], &mut shell)
            .is_success());
        assert!(builtin_exists(&["ion".into(), "-b".into(), "empty_file".into()], &mut shell)
            .is_failure());
        assert!(builtin_exists(
            &["ion".into(), "-b".into(), "file_does_not_exist".into()],
            &mut shell
        )
        .is_failure());

        // restore original PATH. Not necessary for the currently defined test cases
        // but this might change in the future? Better safe than sorry!
        shell.variables_mut().set("PATH", oldpath);

        // check `exists -d`
        // no argument means we treat it as a string
        assert!(builtin_exists(&["ion".into(), "-d".into()], &mut shell).is_success());
        assert!(builtin_exists(&["ion".into(), "-d".into(), "testing/".into()], &mut shell)
            .is_success());
        assert!(builtin_exists(
            &["ion".into(), "-d".into(), "testing/empty_file".into()],
            &mut shell
        )
        .is_failure());
        assert!(builtin_exists(&["ion".into(), "-d".into(), "does/not/exist/".into()], &mut shell)
            .is_failure());

        // check `exists -f`
        // no argument means we treat it as a string
        assert!(builtin_exists(&["ion".into(), "-f".into()], &mut shell).is_success());
        assert!(builtin_exists(&["ion".into(), "-f".into(), "testing/".into()], &mut shell)
            .is_failure());
        assert!(builtin_exists(
            &["ion".into(), "-f".into(), "testing/empty_file".into()],
            &mut shell
        )
        .is_success());
        assert!(builtin_exists(&["ion".into(), "-f".into(), "does-not-exist".into()], &mut shell)
            .is_failure());

        // check `exists -s`
        // no argument means we treat it as a string
        assert!(builtin_exists(&["ion".into(), "-s".into()], &mut shell).is_success());
        shell.variables_mut().set("emptyvar", "".to_string());
        assert!(builtin_exists(&["ion".into(), "-s".into(), "emptyvar".into()], &mut shell)
            .is_failure());
        shell.variables_mut().set("testvar", "foobar".to_string());
        assert!(
            builtin_exists(&["ion".into(), "-s".into(), "testvar".into()], &mut shell).is_success()
        );
        shell.variables_mut().remove("testvar");
        assert!(
            builtin_exists(&["ion".into(), "-s".into(), "testvar".into()], &mut shell).is_failure()
        );
        // also check that it doesn't trigger on arrays
        let mut array = types::Array::new();
        array.push("element".into());
        shell.variables_mut().remove("array");
        shell.variables_mut().set("array", array);
        assert!(
            builtin_exists(&["ion".into(), "-s".into(), "array".into()], &mut shell).is_failure()
        );

        // check `exists --fn`
        let name_str = "test_function";
        let name = types::Str::from(name_str);
        let mut args = Vec::new();
        args.push(KeyBuf { name: "testy".into(), kind: Primitive::Str });
        let mut statements = Vec::new();
        statements.push(Statement::End);
        let description: types::Str = "description".into();

        shell.variables_mut().set(
            &name,
            Value::Function(Function::new(Some(description), name.clone(), args, statements)),
        );

        assert!(builtin_exists(&["ion".into(), "--fn".into(), name_str.into()], &mut shell)
            .is_success());
        shell.variables_mut().remove(name_str);
        assert!(builtin_exists(&["ion".into(), "--fn".into(), name_str.into()], &mut shell)
            .is_failure());

        // check invalid flags / parameters (should all be treated as strings and
        // therefore succeed)
        assert!(builtin_exists(&["ion".into(), "--foo".into()], &mut shell).is_success());
        assert!(builtin_exists(&["ion".into(), "-x".into()], &mut shell).is_success());
    }

    #[test]
    fn test_match_flag_argument() {
        let shell = Shell::default();

        // we don't really care about the passed values, as long as both sited return
        // the same value
        assert_eq!(
            match_flag_argument('a', "ARRAY", &shell),
            array_var_is_not_empty("ARRAY", &shell)
        );
        assert_eq!(match_flag_argument('b', "binary", &shell), binary_is_in_path("binary", &shell));
        assert_eq!(match_flag_argument('d', "path", &shell), path_is_directory("path"));
        assert_eq!(match_flag_argument('f', "file", &shell), path_is_file("file"));
        assert_eq!(match_flag_argument('s', "STR", &shell), string_var_is_not_empty("STR", &shell));

        // Any flag which is not implemented
        assert_eq!(match_flag_argument('x', "ARG", &shell), false);
    }

    #[test]
    fn test_match_option_argument() {
        let shell = Shell::default();

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
        let mut shell = Shell::default();

        // TODO: We should probably also test with more complex PATH-variables:
        // TODO: multiple/:directories/
        // TODO: PATH containing directories which do not exist
        // TODO: PATH containing directories without read permission (for user)
        // TODO: PATH containing directories without execute ("enter") permission (for
        // user) TODO: empty PATH?
        shell.variables_mut().set("PATH", "testing/".to_string());

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
    fn test_array_var_is_not_empty() {
        let mut shell = Shell::default();

        shell.variables_mut().set("EMPTY_ARRAY", types::Array::new());
        assert_eq!(array_var_is_not_empty("EMPTY_ARRAY", &shell), false);

        let mut not_empty_array = types::Array::new();
        not_empty_array.push("array not empty".into());
        shell.variables_mut().set("NOT_EMPTY_ARRAY", not_empty_array);
        assert_eq!(array_var_is_not_empty("NOT_EMPTY_ARRAY", &shell), true);

        // test for array which does not even exist
        shell.variables_mut().remove("NOT_EMPTY_ARRAY");
        assert_eq!(array_var_is_not_empty("NOT_EMPTY_ARRAY", &shell), false);

        // array_var_is_not_empty should NOT match for non-array variables with the
        // same name
        shell.variables_mut().set("VARIABLE", "notempty-variable");
        assert_eq!(array_var_is_not_empty("VARIABLE", &shell), false);
    }

    #[test]
    fn test_string_var_is_not_empty() {
        let mut shell = Shell::default();

        shell.variables_mut().set("EMPTY", "");
        assert_eq!(string_var_is_not_empty("EMPTY", &shell), false);

        shell.variables_mut().set("NOT_EMPTY", "notempty");
        assert_eq!(string_var_is_not_empty("NOT_EMPTY", &shell), true);

        // string_var_is_not_empty should NOT match for arrays with the same name
        let mut array = types::Array::new();
        array.push("not-empty".into());
        shell.variables_mut().set("ARRAY_NOT_EMPTY", array);
        assert_eq!(string_var_is_not_empty("ARRAY_NOT_EMPTY", &shell), false);

        // test for a variable which does not even exist
        shell.variables_mut().remove("NOT_EMPTY");
        assert_eq!(string_var_is_not_empty("NOT_EMPTY", &shell), false);
    }

    #[test]
    fn test_function_is_defined() {
        let mut shell = Shell::default();

        // create a simple dummy function
        let name_str = "test_function";
        let name: types::Str = name_str.into();
        let mut args = Vec::new();
        args.push(KeyBuf { name: "testy".into(), kind: Primitive::Str });
        let mut statements = Vec::new();
        statements.push(Statement::End);
        let description: types::Str = "description".into();

        shell.variables_mut().set(
            &name,
            Value::Function(Function::new(Some(description), name.clone(), args, statements)),
        );

        assert_eq!(function_is_defined(name_str, &shell), true);
        shell.variables_mut().remove(name_str);
        assert_eq!(function_is_defined(name_str, &shell), false);
    }
}
