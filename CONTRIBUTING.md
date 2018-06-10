## Find an issue

First, find an area to work on within the shell or one of it's related projects.
This may be:

- An existing issue which has been reported
- A feature that is missing that you would like to develop
- An issue you've discovered that you would like to fix

Once you have found something that you want to work on, submit your intent to
the issue board, either by creating an issue for an issue which does not exist,
or commenting on an issue that you are working on it.

## Test your code

Before submitting a PR, ensure that you've run your tests locally and that they
pass. This can be done by running the following two commands:

```sh
cargo +nightly test --lib && bash examples/run_examples/sh
```

## Format your code

In addition, format your code before submitting a PR. This will require that
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
