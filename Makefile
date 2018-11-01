prefix ?= usr/local
BINARY = $(prefix)/bin/ion
RELEASE = debug
DEBUG ?= 0
VENDORED = 0

ifeq (0,$(DEBUG))
	ARGS += --release
	RELEASE = release
endif

ifeq (1,$(REDOX))
	ARGS += --target x86_64-unknown-redox
endif

ifneq ($(wildcard vendor.tar.xz),)
	VENDORED = 1
	ARGSV += --frozen
endif

.PHONY: all clean distclean install uninstall

all: extract .cargo/config
	cargo build $(ARGS) $(ARGSV)

clean:
	cargo clean

distclean:
	rm -rf vendor vendor.tar.xz .cargo

tests:
	cargo test $(ARGSV)
	bash examples/run_examples.sh
	for crate in members/*; do \
		cargo test $(ARGSV) --manifest-path $$crate/Cargo.toml; \
	done

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

extract:
ifeq (1,$(VENDORED)$(wildcard vendor))
	tar pxf vendor.tar.xz
endif

update-shells:
	if ! grep ion /etc/shells >/dev/null; then \
		echo $(BINARY) >> /etc/shells; \
	else \
		shell=$(shell grep ion /etc/shells); \
		if [ $$shell != $(BINARY) ]; then \
			sed -i -e "s#$$shell#$(BINARY)#g" /etc/shells; \
		fi \
	fi
