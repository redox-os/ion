# Introduction

Ion is a modern system shell that features a simple, yet powerful, syntax. It is written entirely
in Rust, which greatly increases the overall quality and security of the shell. It also offers a
level of performance that exceeds that of Dash, when taking advantage of Ion's features. While it
is developed alongside, and primarily for, RedoxOS, it is a fully capable on other \*nix platforms.

# Ion Shell

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![crates.io](https://meritbadge.herokuapp.com/ion-shell)](https://crates.io/crates/ion-shell)
[![Documentation](https://img.shields.io/badge/documentation-blue)](https://doc.redox-os.org/ion-manual)

> Ion is still a WIP, and both its syntax and rules are subject to change over time. It is
> still quite a ways from becoming stabilized, but we are getting very close. Changes to the
> syntax at this time are likely to be minimal.

# Ion Specification

Ion has a RFC process for language proposals. Ion's formal specification is located within the
[rfcs](https://gitlab.redox-os.org/redox-os/ion/tree/rfcs) branch. The RFC process is still in
the early stages of development, so much of the current and future implementation ideas have
yet to be written into the specification.

# Ion Manual

[The Ion manual online](https://doc.redox-os.org/ion-manual) 
is generated automatically on each commit via [mdBook](https://github.com/azerupi/mdBook) and hosted on Redox OS's website.

**Building the manual for local reference**

Sources for the manual are located in the `manual` directory.

1. Build the documentation file for the builtins 

```sh
make manual
```

2. Then build the rest of the Ion manual via mdbook

```sh
mdbook build manual
```

Or you can build and open it in the your default browser via 

```sh
mdbook serve manual --open
```

Or you can build and host the manual on your localhost via

```sh
mdbook serve manual 
```

# Ion library example

See the [examples folder](https://gitlab.redox-os.org/redox-os/ion/tree/master/examples) and the [Parallelion project](https://gitlab.redox-os.org/AdminXVII/parallelion)

# Packages

## Pop!\_OS / Ubuntu

The following PPA supports the 18.04 (bionic) and 19.04 (disco) releases. Bionic builds were made using the Pop\_OS PPA's rustc 1.39.0 package.

```
sudo add-apt-repository ppa:mmstick76/ion-shell
```

# Developer set up

Those who are developing software with Rust should install the [Rustup toolchain manager](https://rustup.rs/).
After installing rustup, run `rustup override set 1.56.0` to set your Rust toolchain to the version that Ion is
targeting at the moment. To build for Redox OS, `rustup override set nightly` is required to build the Redox
dependencies.

# Build dependencies

Please ensure that both cargo and rustc 1.56.0 or higher is installed for your system.
Release tarballs have not been made yet due to Ion being incomplete in a few remaining areas.

# Compile instructions for distribution

```sh
git clone https://gitlab.redox-os.org/redox-os/ion/
cd ion
RUSTUP=0 make # By default RUSTUP equals 1, which is for developmental purposes
sudo make install prefix=/usr
sudo make update-shells prefix=/usr
```

> To compile in DEBUG mode, pass `DEBUG=1` as an argument to `make`

# Ion plugins

There are plugins for ion. These plugins are additional aliases and function definitions written in 
Ion for Ion. They can be found under this [repository](https://gitlab.redox-os.org/redox-os/ion-plugins).

# Vim/NeoVim Syntax Highlighting Plugin

For vim/nvim users there is an [officially-supported syntax highlighting plugin](https://gitlab.redox-os.org/redox-os/ion-vim).

```vimscript
Plugin 'vmchale/ion-vim'
```

![Vim Syntax Highlighting](.gitlab/vim_syntax.png)

# Emacs Syntax Highlighting Plugin

For emacs users there is a [kindly-supported syntax highlighting plugin](https://github.com/iwahbe/ion-mode).

```emacs
(add-to-list 'load-path  (expand-file-name "/path/to/ion-mode"))
(require 'ion-mode)
(autoload 'ion-mode (locate-library "ion-mode") "Ion majore mode" t)
(add-to-list 'auto-mode-alist '("\\.ion\\'" . ion-mode))
(add-to-list 'auto-mode-alist '("/ion/initrc" . ion-mode))
```

![Emacs Syntax Highlighting](.gitlab/emacs_syntax.png)
