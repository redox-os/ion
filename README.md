# Ion Shell

[![Build Status](https://travis-ci.org/redox-os/ion.svg)](https://travis-ci.org/redox-os/ion)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](http://meritbadge.herokuapp.com/ion-shell)](https://crates.io/crates/ion-shell)
![LOC](https://tokei.rs/b1/github/redox-os/ion)

# New Ion MdBook

We are providing our manual for Ion in the form of a markdown-based book, which is accessible via:

- The official page for Ion's manual on Redox's website: https://doc.redox-os.org/ion-manual/
- Installing the mdbook via our `setup.ion` script and having Ion open an offline copy via `ion-docs`.
- Building and serving the book in the **manual** directory in the yourself with [mdBook](https://github.com/azerupi/mdBook)

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

# Why Not POSIX?

If Ion had to follow POSIX specifications, it wouldn't be half the shell that it is today, and
there'd be no solid reason to use Ion over any other existing shell, given that it'd basically be
the same as every other POSIX shell. Redox OS itself doesn't follow POSIX specifications, and
neither does it require a POSIX shell for developing Redox's userspace. It's therefore not meant
to be used as a drop-in replacement for Dash or Bash. You should retain Dash/Bash on your system
for execution of Dash/Bash scripts, but you're free to write new scripts for Ion, or use Ion as
the interactive shell for your user session. Redox OS, for example, also contains Dash for
compatibility with software that depends on POSIX scripts.

That said, Ion's foundations are heavily inspired by POSIX shell syntax. If you have experience
with POSIX shells, then you already have a good idea of how most of Ion's core features operate. A
quick sprint through this documentation will bring you up to speed on the differences between our
shell and POSIX shells. Namely, we carry a lot of the same operators: **$**, **|**, **||**, **&**,
**&&**, **>**, **<**, **<<**, **<<<**, **$()**, **$(())**.  Yet we also offer some functionality
of our own, such as **@**, **@()**, **$method()**, **@method()**, **^|**, **^>**, **&>**, **&|**.
Essentially, we have taken the best components of the POSIX shell specifications, removed the bad
parts, and implemented even better features on top of the best parts. That's how open source
software evolves: iterate, deploy, study, repeat.

# Compile / Install Instructions

Rust 1.19 is the minimum requirement for compiling Ion. Simplest way to obtain Rust/Cargo is by
installing the [Rustup toolchain manager](https://rustup.rs/), in the event that your OS does
not ship Rust natively, or if you want more flexibility in Rust compilation capabilities.

Then, it's just a matter of performing one of the following methods:

## Install Latest Stable Version From Crates.io

Use the `--force` flag when updating a binary that's already installed with cargo.

```sh
cargo install ion-shell
```

## Install Direct From Git

```sh
cargo install --git https://github.com/redox-os/ion/
```

## Build Locally

```sh
git clone https://github.com/redox-os/ion/
cd ion
cargo build --release
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
