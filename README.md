# Ion Shell

[![Build Status](https://travis-ci.org/redox-os/ion.svg)](https://travis-ci.org/redox-os/ion)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](http://meritbadge.herokuapp.com/ion-shell)](https://crates.io/crates/ion-shell)
![LOC](https://tokei.rs/b1/github/redox-os/ion)

> Ion is still a WIP, and both it's syntax and rules are subject to change over time. It is
> still quite a ways from becoming stabilized, but we are getting very close. Changes to the
> syntax at this time are likely to be minimal.

# Ion Manual

We are providing our manual for Ion in the form of a markdown-based book, which is accessible via:

- The official page for Ion's manual on Redox's website: https://doc.redox-os.org/ion-manual/
- Installing the mdbook via our `setup.ion` script and having Ion open an offline copy via `ion-docs`.
- Building and serving the book in the **manual** directory yourself with [mdBook](https://github.com/azerupi/mdBook)

> Note, however, that the manual is incomplete, and does not cover all of Ion's functionality
> at this time. Anyone willing to help with documentation should simply do so and submit a pull
> request. If you have any questions regarding certain implementation details, feel free to
> ask in whichever venue you are most comfortable with.

# Contributors

## Guidelines

### Code Formatting

When submitting a pull request, be sure to run
`env CFG_RELEASE_CHANNEL=nightly cargo +nightly fmt` on your project with a
nightly version of **rustfmt**. This will prevent me from having to push PR's specifically
to format the code base from time to time. To install **rustfmt-nightly**, simply run
`cargo install rustfmt-nightly --force`.

### On Unit & Integration Tests

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

## Issue Board

Please visit the [issue board](https://github.com/redox-os/ion/issuesi) for a list of curated
issues that need to be worked on. If an issue has the **WIP** tag, then that issue is currently
being worked on. Otherwise, the issue is free game. Issues are also labeled to guide contributors
into finding problems to tackle at a given skill level. Not to worry though, because most issues
within Ion are relatively simple **C-Class** issues. The most difficult issues are marked as
**A-Class**.

## Chatroom

Send an email to [info@redox-os.org](mailto:info@redox-os.org) to request invitation for joining
the developer chatroom for Ion. Experience with Rust is not required for contributing to Ion. There
are ways to contribute to Ion at all levels of experience, from writing scripts in Ion and reporting
issues, to seeking mentorship on how to implement solutions for specific issues on the issue board.

## Discussion

In addition to the chatroom, there's a [thread in the Redox forums](https://discourse.redox-os.org/t/ion-shell-development-discussion/682)
that can be used for discussions relating to Ion and Ion shell development. These are mostly served
by the GitHub issue board, but general discussions can take place there instead.

# Introduction

Ion is a modern system shell that features a simple, yet powerful, syntax. It is written entirely
in Rust, which greatly increases the overall quality and security of the shell, eliminating the
possibilities of a [ShellShock](http://www.wikiwand.com/en/Shellshock_(software_bug))-like vulnerability
, and making development easier. It also offers a level of performance that exceeds that of Dash,
when taking advantage of Ion's features. While it is developed alongside, and primarily for, RedoxOS,
it is a fully capable on other \*nix platforms.

## Why Not Windows Support?

Windows is not, and may never be supported due to certain limitations in the NT kernel. Namely,
where in all non-Windows operating systems, the kernel takes an array of strings that defines
the command to execute, and all of that command's arguments; Windows instead takes a single
string that contains both the command and all of it's arguments. This pushes the job of parsing
arguments from the system shell onto the individual program, and may account for why the command-line
in Windows is so funky.

In addition, Windows does not support forking, a concept by which a new sub-process is spawned with
the same state as the parent, for the purpose of continuing execution down a different path from the
parent. This enables for subshells to be spawned, as commonly seen by process expansions (**$()**),
among piping builtins and functions.

# Goals

Syntax and feature decisions for Ion are made based upon three measurements: is the feature useful,
is it simple to use, and will it's implementation be efficient to parse and execute? A feature is
considered useful if there's a valid use case for it, in the concept of a shell language. The
syntax for the feature should be simple for a human to read and write, with extra emphasis on
readability, given that most time is spent reading scripts than writing them. The implementation
should require minimal to zero heap allocations, and be implemented in a manner that requires
minimal CPU cycles (so long as it's also fully documented and easy to maintain!).

It should also be taken into consideration that shells operate entirely upon strings, and therefore
should be fully equipped for all manner of string manipulation capabilities. That means that users
of a shell should not immediately need to grasp for tools like **cut**, **sed**, and **awk**. Ion
offers a great deal of control over slicing and manipulating text. Arrays are treated as first
class variables with their own unique **@** sigil. Strings are also treated as first class
variables with their own unique **$** sigil. Both support being sliced with **[range]**, and they
each have their own supply of methods.

# Compile / Install Instructions

Rust nightly is required for compiling Ion. Simplest way to obtain Rust/Cargo is by
installing the [Rustup toolchain manager](https://rustup.rs/), in the event that your OS does
not ship Rust natively, or if you want more flexibility in Rust compilation capabilities.

Then, it's just a matter of performing one of the following methods:

## Install Direct From Git

```sh
cargo install --git https://github.com/redox-os/ion/
```

## Build Locally

```sh
git clone https://github.com/redox-os/ion/
cd ion && cargo build --release
```

# Git Plugin

Plugins support within Ion is still a work in progress, and so the plugin architecture is likely to change. That said,
there's an official git plugin that can be installed to experiment with the existing plugin namespaces plugin support.
To install the git plugin, first install ion, and then execute the following:

```ion
./setup.ion install plugins
```

It can be tested out by navigating to a directory within a git repository, and running the following:

```ion
echo Current Branch: ${git::branch}${git::modified_count}${git::untracked_count}
```

# Vim/NeoVim Syntax Highlighting Plugin

We do have an [officially-supported syntax highlighting plugin](https://github.com/vmchale/ion-vim) for all the
vim/nvim users out there.

```vimscript
Plugin 'vmchale/ion-vim'
```

![Screenshot of Syntax Highlighting](http://i.imgur.com/JzZp7WT.png)
