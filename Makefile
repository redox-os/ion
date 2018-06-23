DESTDIR ?= /usr/local/bin/

all:
	cargo build --release

tests:
	cargo test --manifest-path members/braces/Cargo.toml
	cargo test --manifest-path members/builtins/Cargo.toml
	cargo test --manifest-path members/lexers/Cargo.toml
	cargo test --manifest-path members/ranges/Cargo.toml
	bash examples/run_examples.sh

install:
	strip -s target/release/ion
	install -Dm0755 target/release/ion $(DESTDIR)

uninstall:
	rm $(DESTDIR)/ion
