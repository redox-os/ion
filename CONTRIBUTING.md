# Contributor Guidelines

Contributors MUST:
Comply with the templates using [conventional commit](https://www.conventionalcommits.org/en/v1.0.0-beta.4/) or **explicitly reason why not**

## Merge Requests

Contributors MUST:

- Comply with the templates using [conventional commit](https://www.conventionalcommits.org/en/v1.0.0-beta.4/) or **explicitly reason why not**
- For **bug fixes** fill 1. Description, 2.Related issue, 3.Regression test, 4.Refactoring statement, 6.Documentation and 7.Performance
- For **features** fill 1. Description, 2.Related discussion, 3.Unit test, 4. Integration test, 5. Refactoring statement, 6.Documentation and 7.Performance
- For **BREAKING CHANGE**, where valid programs are not working anymore, create a Request For Comment(RFC)
- Format your code with `cargo +nightly fmt` before creating a commit
- Squash commits, such that each commit clearly does a specific thing, either locally or using gitlab's custom checkbox.
- [Adhere to a git workflow using rebase](https://medium.com/singlestone/a-git-workflow-using-rebase-1b1210de83e5)
- Rebase upon the master branch, rather than merging it
- [Allow us to make commits on your merge request](https://docs.gitlab.com/ee/user/project/merge_requests/allow_collaboration.html)

Contributors MUST NOT:

- Have merge commits in their merge requests
- Have breaking changes without RFC
- Have commits which do not adhere to [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0-beta.4/) guidelines

Contributors SHOULD NOT:

- Worry about code style, because `cargo fmt` renders this a non-issue

[conventional commit]: https://www.conventionalcommits.org/en/v1.0.0-beta.4/

## Finding an issue

1. Find an area to work on within the shell or one of it's related projects.
This may be:

- An existing issue which has been reported
- A feature that is missing that you would like to develop
- An issue you've discovered that you would like to fix

2. Submit your intent to the issue board. Write into an existing issue or create a new issue.

## On Unit & Integration Tests

Feature addition to the code should be accompanied by unit and integration tests,
in order to prevent issues from creating on refactoring in the future.
Bug fixes should be combined with regression tests in order to prevent issues from 
re-occuring in the future.

If you see an area that deserves a test, feel free to add extra tests in your pull requests.
When submitting new functionality, especially complex functionality, try to write as many
tests as you can think of to cover all possible code paths that your function(s) might take.
Integration tests are located in the **tests** directory, and are the most important place
to create tests -- unit tests come second after the integration tests.
Regression tests are both integration and unit tests, depending on the bug.

Integration tests are much more useful in general, as they cover real world use cases and
stress larger portions of the code base at once. 
See [this section][integration test] on how integration tests are done for Ion.

Yet unit tests still have their place, as
they are able to test bits of functionality which may not necessarily be covered by existing
integration tests.

> In order to create unit tests for otherwise untestable code that depends on greater runtime
> specifics, you should likely write your functions to accept generic inputs, where unit
> tests can pass dummy types and environments into your functions for the purpose of testing
> the function, whereas in practice the function is hooked up to it's appropriate types.


## Test your code

Before submitting a merge request (MR) on GitLab, ensure that you've run your tests locally and that they
pass.

You can all tests via this command.

```sh
make tests
```

## Format your code

In addition, format your code before submitting a MR. This will require that
you've installed the `rustfmt` Cargo component.

```sh
cargo +nightly fmt
```

Now you're ready to submit your work for review!

## Submitting your work for review

Submitting your work on the Redox OS GitLab server can be done by creating a [merge request (MR)](https://gitlab.redox-os.org/help/user/project/merge_requests/index.md).

**Important** Make sure you [enable commit edits from upstream members](https://gitlab.redox-os.org/help/user/project/merge_requests/allow_collaboration.md#enabling-commit-edits-from-upstream-members) by clicking the *"Allow commits from members who can merge to the target branch"* checkbox.

## Chatroom

You can join the chat of redox on maxtrix under https://matrix.to/#/#redox:matrix.org which is used by the developers too. 
Experience with Rust is not required for contributing to Ion. There
are ways to contribute to Ion at all levels of experience, from writing scripts in Ion and reporting
issues, to seeking mentorship on how to implement solutions for specific issues on the issue board.

## Discussion

In addition to the chatroom, there's a [thread in the Redox forums](https://discourse.redox-os.org/t/ion-shell-development-discussion/682)
that can be used for discussions relating to Ion and Ion shell development. These are mostly served
by the GitHub issue board, but general discussions can take place there instead.

## How integration tests work in Ion

Integration tests are located at the folder named "tests" which is relative to the project root. 
This is usual for rust projects. 
Ion however does integration test via Ion/parameter input and expected output files.

- An input is just an Ion or parameter file which is executed by the Ion shell. 
  Such a file has the file extension "ion" or "params".
- The expected output file is a normal text file 
  which contains the expected content of the integration test.
  Such a file has the file extension "out". 

This [bash script] executes all the Ion input files with the Ion shell and compares
their output with their corresponding output files.

---

### How to create an integration test in general

There 2 ways of integration tests.

1. Ion executes an Ion script file
2. Ion is executed with arguments

You need to create an input file and an expected output file under the folder "tests".
The base name of the input file and its respective expected output file needs to be same.
Only the file extension of the both files should differ. 
Example: integration test "example_method" would have an input file "example_method.ion" 
and an expected output file "example_method.out" under the folder "tests".

---

### How to create integration test where Ion executes an Ion file

To create an integration test which is named "first_integration_test" 
and is conducted by executing an Ion script file.

1. Create a file named "first_integration_test.ion" 
   with the content which Ion shell should execute
2. Create a file named "first_integration_test.out" 
   with the expected output 

--- 

### How to create integration test where Ion is executed with arguments

To create an integration test which is named "first_integration_test" 
and is conducted by executing the Ion shell with certain arguments.

1. Create a file named "first_integration_test.params"
   with the arguments which the Ion shell should be executed with.
2. Create a file named "first_integration_test.out"
   with the expected output

Every option and value for an option goes into a new line. 
See [params file](./tests/keybinding_fail.params) as an example.
This example corresponds to the following command which is executed for the integration test

```sh
ion -o viemacs -c "echo 1"
```

[bash script]:./tests/run_examples.sh
[integration test]:#how-integration-tests-work-in-ion
