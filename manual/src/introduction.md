# Introduction

Ion is a modern system shell that features a simple, yet powerful, syntax. It is written entirely
in Rust, which greatly increases the overall quality and security of the shell, eliminating the
possibilities of a [ShellShock](https://en.wikipedia.org/wiki/Shellshock_(software_bug))-like vulnerability, and making development easier. It also
offers a level of performance that exceeds that of Dash, while taking advantage of Ion's features.
While it is developed alongside, and primarily for, RedoxOS, it is fully capable of running on other *nix
platforms, and we are currently searching for a Windows developer to port it to Windows.

# Goals

Syntax and feature decisions for Ion are made based upon three measurements: 
 1. Is the feature useful?
 2. Is it simple to use?
 3. Will its implementation be efficient to parse and execute? 

A feature is considered useful if there's a valid use case for it, in the context of a shell language. The
syntax for the feature should be simple for a human to read and write, with extra emphasis on
readability, given that **more time is spent reading scripts** than writing them. The implementation
should require *minimal to zero heap allocations*, and be implemented in a manner that requires
*minimal CPU cycles* (so long as it's also **fully documented** and **easy to maintain**!).

It should also be taken into consideration that *shells operate entirely upon strings*, and therefore
should be fully equipped for all manner of string manipulation capabilities. That means that users
of a shell should not immediately need to reach for tools like **cut**, **sed**, and **awk**. Ion
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
