prefix ?= /usr/local
BINARY = $(shell echo $(prefix)/bin/ion | sed 's/\/\//\//g')

all:
	cargo build --release

tests:
	cargo test --manifest-path members/braces/Cargo.toml
	cargo test --manifest-path members/builtins/Cargo.toml
	cargo test --manifest-path members/lexers/Cargo.toml
	cargo test --manifest-path members/ranges/Cargo.toml
	cargo test 
	bash examples/run_examples.sh

install: update-shells
	install -Dm0755 target/release/ion $(DESTDIR)/$(BINARY)

uninstall:
	rm $(DESTDIR)/$(BINARY)

update-shells:
	if ! grep ion /etc/shells >/dev/null; then \
		echo $(BINARY) >> /etc/shells; \
	else \
		shell=$(shell grep ion /etc/shells); \
		if [ $$shell != $(BINARY) ]; then \
			before=$$(echo $$shell | sed 's/\//\\\//g'); \
			after=$$(echo $(BINARY) | sed 's/\//\\\//g'); \
			sed -i -e "s/$$before/$$after/g" /etc/shells; \
		fi \
	fi
