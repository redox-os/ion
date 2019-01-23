prefix ?= usr/local
BINARY = $(prefix)/bin/ion
RELEASE = debug
DEBUG ?= 0
VENDORED = 0
REDOX ?= 0

ifeq (0,$(DEBUG))
	ARGS += --release
	RELEASE = release
endif

ifneq ($(wildcard vendor.tar.xz),)
	ARGSV += --frozen
endif

ifeq (1,$(REDOX))
	undefine ARGSV
	ARGS += --target x86_64-unknown-redox
	PREARGS += +nightly
endif

.PHONY: all clean distclean install uninstall

all: version extract .cargo/config
	cargo $(PREARGS) build $(ARGS) $(ARGSV)

clean:
	cargo clean

distclean: clean
	rm -rf vendor vendor.tar.xz .cargo git_revision.txt

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
	echo $(wildcard vendor.tar.xz)
	if [ "$(wildcard vendor.tar.xz)" != "" ]; then \
		cp vendor_config $@; \
	else \
		cp nonvendor_config $@; \
	fi

vendor.tar.xz:
	cargo vendor
	tar pcfJ vendor.tar.xz vendor
	rm -rf vendor

vendor: vendor.tar.xz .cargo/config

extract:
ifneq ($(wildcard vendor.tar.xz),)
ifneq (1,$(REDOX))
	tar pxf vendor.tar.xz
endif
endif

version:
ifeq ($(wildcard git_revision.txt),)
	git rev-parse master > git_revision.txt
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

