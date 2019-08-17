## Find an issue

First, find an area to work on within the shell or one of it's related projects.
This may be:

- An existing issue which has been reported
- A feature that is missing that you would like to develop
- An issue you've discovered that you would like to fix

Once you have found something that you want to work on, submit your intent to
the issue board, either by creating an issue for an issue which does not exist,
or commenting on an issue that you are working on it.

## On Unit & Integration Tests

Changes made to the code should normally be accompanied by both unit and integration tests,
in order to prevent these issues from re-occuring in the future.

If you see an area that deserves a test, feel free to add extra tests in your pull requests.
When submitting new functionality, especially complex functionality, try to write as many
tests as you can think of to cover all possible code paths that your function(s) might take.
Integration tests are located in the **examples** directory, and are the most important place
to create tests -- unit tests come second after the integration tests.

Integration tests are much more useful in general, as they cover real world use cases and
stress larger portions of the code base at once. Yet unit tests still have their place, as
they are able to test bits of functionality which may not necessarily be covered by existing
integration tests.

> In order to create unit tests for otherwise untestable code that depends on greater runtime
> specifics, you should likely write your functions to accept generic inputs, where unit
> tests can pass dummy types and environments into your functions for the purpose of testing
> the function, whereas in practice the function is hooked up to it's appropriate types.

## Test your code

Before submitting a merge request (MR) on GitLab, ensure that you've run your tests locally and that they
pass. This can be done by running the following two commands:

```sh
cargo +nightly test --lib && bash examples/run_examples.sh
```

## Format your code

In addition, format your code before submitting a MR. This will require that
you've installed the `rustfmt` Cargo component.

```sh
cargo +nightly fmt
```

Now you're ready to submit your work for review!

## Chatroom

Send an email to [info@redox-os.org](mailto:info@redox-os.org) to request invitation for joining
the developer chatroom for Ion. Experience with Rust is not required for contributing to Ion. There
are ways to contribute to Ion at all levels of experience, from writing scripts in Ion and reporting
issues, to seeking mentorship on how to implement solutions for specific issues on the issue board.

## Discussion

In addition to the chatroom, there's a [thread in the Redox forums](https://discourse.redox-os.org/t/ion-shell-development-discussion/682)
that can be used for discussions relating to Ion and Ion shell development. These are mostly served
by the GitHub issue board, but general discussions can take place there instead.
