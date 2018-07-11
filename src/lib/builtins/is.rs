use shell::Shell;
use types;
use small;

pub(crate) fn is(args: &[small::String], shell: &mut Shell) -> Result<(), String> {
    match args.len() {
        4 => if args[1] != "not" {
            return Err(format!("Expected 'not' instead found '{}'\n", args[1]).to_string());
        } else if eval_arg(&*args[2], shell) == eval_arg(&*args[3], shell) {
            return Err("".to_string());
        },
        3 => if eval_arg(&*args[1], shell) != eval_arg(&*args[2], shell) {
            return Err("".to_string());
        },
        _ => return Err("is needs 3 or 4 arguments\n".to_string()),
    }

    Ok(())
}

fn eval_arg(arg: &str, shell: &mut Shell) -> types::Str {
    let value = get_var_string(arg, shell);
    if &*value != "" {
        return value;
    }
    arg.into()
}

// On error returns an empty String.
fn get_var_string(name: &str, shell: &mut Shell) -> types::Str {
    if name.chars().nth(0).unwrap() != '$' {
        return "".into();
    }

    match shell.variables.get::<types::Str>(&name[1..]) {
        Some(s) => s,
        None => "".into(),
    }
}

#[test]
fn test_is() {
    fn vec_string(args: &[&str]) -> Vec<small::String> {
        args.iter().map(|s| (*s).into()).collect()
    }
    use shell::ShellBuilder;
    let mut shell = ShellBuilder::new().as_library();
    shell.set("x", "value");
    shell.set("y", "0");

    // Four arguments
    assert_eq!(
        is(&vec_string(&["is", " ", " ", " "]), &mut shell),
        Err("Expected 'not' instead found ' '\n".to_string())
    );
    assert_eq!(
        is(&vec_string(&["is", "not", " ", " "]), &mut shell),
        Err("".to_string())
    );
    assert_eq!(
        is(&vec_string(&["is", "not", "$x", "$x"]), &mut shell),
        Err("".to_string())
    );
    assert_eq!(is(&vec_string(&["is", "not", "2", "1"]), &mut shell), Ok(()));
    assert_eq!(is(&vec_string(&["is", "not", "$x", "$y"]), &mut shell), Ok(()));

    // Three arguments
    assert_eq!(is(&vec_string(&["is", "1", "2"]), &mut shell), Err("".to_string()));
    assert_eq!(is(&vec_string(&["is", "$x", "$y"]), &mut shell), Err("".to_string()));
    assert_eq!(is(&vec_string(&["is", " ", " "]), &mut shell), Ok(()));
    assert_eq!(is(&vec_string(&["is", "$x", "$x"]), &mut shell), Ok(()));

    // Two arguments
    assert_eq!(
        is(&vec_string(&["is", " "]), &mut shell),
        Err("is needs 3 or 4 arguments\n".to_string())
    );

    // One argument
    assert_eq!(
        is(&vec_string(&["is"]), &mut shell),
        Err("is needs 3 or 4 arguments\n".to_string())
    );
}
