// TODO: Move into grammar


use shell::status::*;
use shell::variables::Variables;

/// Dropping an alias will erase it from the shell.
pub(crate) fn drop_alias<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
where
    I::Item: AsRef<str>,
{
    let args = args.into_iter().collect::<Vec<I::Item>>();
    if args.len() <= 1 {
        eprintln!("ion: you must specify an alias name");
        return FAILURE;
    }
    for alias in args.iter().skip(1) {
        if vars.aliases.remove(alias.as_ref()).is_none() {
            eprintln!("ion: undefined alias: {}", alias.as_ref());
            return FAILURE;
        }
    }
    SUCCESS
}

/// Dropping an array will erase it from the shell.
pub(crate) fn drop_array<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
where
    I::Item: AsRef<str>,
{
    let args = args.into_iter().collect::<Vec<I::Item>>();
    if args.len() <= 2 {
        eprintln!("ion: you must specify an array name");
        return FAILURE;
    }

    if args[1].as_ref() != "-a" {
        eprintln!("ion: drop_array must be used with -a option");
        return FAILURE;
    }

    for array in args.iter().skip(2) {
        if vars.unset_array(array.as_ref()).is_none() {
            eprintln!("ion: undefined array: {}", array.as_ref());
            return FAILURE;
        }
    }
    SUCCESS
}

/// Dropping a variable will erase it from the shell.
pub(crate) fn drop_variable<I: IntoIterator>(vars: &mut Variables, args: I) -> i32
where
    I::Item: AsRef<str>,
{
    let args = args.into_iter().collect::<Vec<I::Item>>();
    if args.len() <= 1 {
        eprintln!("ion: you must specify a variable name");
        return FAILURE;
    }

    for variable in args.iter().skip(1) {
        if vars.unset_var(variable.as_ref()).is_none() {
            eprintln!("ion: undefined variable: {}", variable.as_ref());
            return FAILURE;
        }
    }

    SUCCESS
}

#[cfg(test)]
mod test {
    use super::*;
    use parser::{expand_string, Expander};
    use shell::status::{FAILURE, SUCCESS};
    use types::*;

    struct VariableExpander(pub Variables);

    impl Expander for VariableExpander {
        fn variable(&self, var: &str, _: bool) -> Option<Value> { self.0.get_var(var) }
    }

    // TODO: Rewrite tests now that let is part of the grammar.
    // #[test]
    // fn let_and_expand_a_variable() {
    //     let mut variables = Variables::default();
    //     let dir_stack = new_dir_stack();
    //     let_(&mut variables, vec!["let", "FOO", "=", "BAR"]);
    // let expanded = expand_string("$FOO", &variables, &dir_stack,
    // false).join("");     assert_eq!("BAR", &expanded);
    // }
    //
    // #[test]
    // fn let_fails_if_no_value() {
    //     let mut variables = Variables::default();
    //     let return_status = let_(&mut variables, vec!["let", "FOO"]);
    //     assert_eq!(FAILURE, return_status);
    // }
    //
    // #[test]
    // fn let_checks_variable_name() {
    //     let mut variables = Variables::default();
    // let return_status = let_(&mut variables, vec!["let", ",;!:", "=",
    // "FOO"]);     assert_eq!(FAILURE, return_status);
    // }

    #[test]
    fn drop_deletes_variable() {
        let mut variables = Variables::default();
        variables.set_var("FOO", "BAR");
        let return_status = drop_variable(&mut variables, vec!["drop", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = expand_string("$FOO", &VariableExpander(variables), false).join("");
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, vec!["drop"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_fails_with_undefined_variable() {
        let mut variables = Variables::default();
        let return_status = drop_variable(&mut variables, vec!["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_deletes_array() {
        let mut variables = Variables::default();
        variables.set_array("FOO", array!["BAR"]);
        let return_status = drop_array(&mut variables, vec!["drop", "-a", "FOO"]);
        assert_eq!(SUCCESS, return_status);
        let expanded = expand_string("@FOO", &VariableExpander(variables), false).join("");
        assert_eq!("", expanded);
    }

    #[test]
    fn drop_array_fails_with_no_arguments() {
        let mut variables = Variables::default();
        let return_status = drop_array(&mut variables, vec!["drop", "-a"]);
        assert_eq!(FAILURE, return_status);
    }

    #[test]
    fn drop_array_fails_with_undefined_array() {
        let mut variables = Variables::default();
        let return_status = drop_array(&mut variables, vec!["drop", "FOO"]);
        assert_eq!(FAILURE, return_status);
    }
}
