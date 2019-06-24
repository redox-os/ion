use ion_shell::{builtins::Status, types, Shell};

/// print 42 to the screen
///
/// SYNOPSIS
///     gimme_the_answer_to_life_to_the_universe_and_to_everything_else [-h | --help]
///
/// DESCRIPTION
///     Who doesn't want 42 printed to screen?
#[builtins_proc::builtin(man = "a comment")]
fn gimme_the_answer_to_life_to_the_universe_and_to_everything_else(
    args: &[types::Str],
    _shell: &mut Shell<'_>,
) -> Status {
    println!("42");
    Status::SUCCESS
}

#[test]
fn works() {
    assert_eq!(
        builtin_gimme_the_answer_to_life_to_the_universe_and_to_everything_else(
            &[],
            &mut Shell::default()
        ),
        Status::SUCCESS
    );
}
