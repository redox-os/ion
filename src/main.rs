extern crate ion_shell;

use ion_shell::{Binary, ShellBuilder};

fn main() {
    ShellBuilder::new()
        .install_signal_handler()
        .block_signals()
        .set_unique_pid()
        .as_binary()
        .main();
}

// TODO: The `Binary` / `main()` logic should be implemented here, and not within the library.
