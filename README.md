# Ion Shell

[![Build Status](https://travis-ci.org/redox-os/ion.svg)](https://travis-ci.org/redox-os/ion)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](http://meritbadge.herokuapp.com/ion-shell)](https://crates.io/crates/ion-shell)
![LOC](https://tokei.rs/b1/github/redox-os/ion)

> Ion is still a WIP, and both it's syntax and rules are subject to change over time. It is
> still quite a ways from becoming stabilized, but we are getting close. Changes to the
> syntax at this time are likely to be minimal.

# Ion Manual

We are providing our manual for Ion in the form of a markdown-based book, which is accessible via:

- The official page for Ion's manual on Redox's website: https://doc.redox-os.org/ion-manual/
- Installing the mdbook via our `setup.ion` script and having Ion open an offline copy via `ion-docs`.
- Building and serving the book in the **manual** directory yourself with [mdBook](https://github.com/azerupi/mdBook)

> Note, however, that the manual is incomplete, and does not cover all of Ion's functionality
> at this time. Anyone willing to help with documentation should request to do so in the chatroom.

# Contributors

Send an email to [info@redox-os.org](mailto:info@redox-os.org) to request invitation for joining
the developer chatroom for Ion. Experience with Rust is not required for contributing to Ion. There
are ways to contribute to Ion at all levels of experience, from writing scripts in Ion and reporting
issues, to seeking mentorship on how to implement solutions for specific issues on the issue board.

# Introduction

Ion is a modern system shell that features a simple, yet powerful, syntax. It is written entirely
in Rust, which greatly increases the overall quality and security of the shell, eliminating the
possibilities of a [ShellShock](http://www.wikiwand.com/en/Shellshock_(software_bug))-like vulnerability, and making development easier. It also
offers a level of performance that exceeds that of Dash, when taking advantage of Ion's features.
While it is developed alongside, and primarily for, RedoxOS, it is a fully capable on other *nix
platforms, and we are currently searching for a Windows developer to port it to Windows.

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
