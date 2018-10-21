prefix ?= usr/local
BINARY = $(prefix)/bin/ion
RELEASE = debug
DEBUG ?= 0

ifeq (0, $(DEBUG))
	ARGS += --release
	RELEASE = release
endif

ifeq (1,$(REDOX))
	ARGS += --target x86_64-unknown-redox
endif

.PHONY: all clean distclean install uninstall

all: .cargo/config
	if [ -f vendor.tar.xz ]; \
	then \
		tar pxf vendor.tar.xz; \
		cargo build $(ARGS) --frozen; \
	else \
		cargo build $(ARGS); \
	fi

clean:
	cargo clean

distclean:
	rm -rf vendor vendor.tar.xz .cargo

tests:
	cargo test --manifest-path members/braces/Cargo.toml
	cargo test --manifest-path members/builtins/Cargo.toml
	cargo test --manifest-path members/lexers/Cargo.toml
	cargo test --manifest-path members/ranges/Cargo.toml
	cargo test 
	bash examples/run_examples.sh

install:
	install -Dm0755 target/$(RELEASE)/ion $(DESTDIR)/$(BINARY)

uninstall:
	rm $(DESTDIR)/$(BINARY)

.cargo/config:
	mkdir -p .cargo
	if [ -f vendor.tar.xz ]; then \
		cp vendor_config $@; \
	else \
		cp nonvendor_config $@; \
	fi \

vendor.tar.xz:
	cargo vendor
	tar pcfJ vendor.tar.xz vendor
	rm -rf vendor

vendor: .cargo/config vendor.tar.xz

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
