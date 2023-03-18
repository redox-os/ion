prefix ?= usr/local
BINARY = $(prefix)/bin/ion
RELEASE = debug
TOOLCHAIN ?= 1.56.0

GIT_REVISION=git_revision.txt
SRC=Cargo.toml Cargo.lock $(shell find src members -type f -wholename '*src/*.rs')
VENDOR=.cargo/config vendor.tar.xz

DEBUG ?= 0
ifeq ($(DEBUG),0)
	ARGS += --release
	RELEASE = release
endif

VENDORED ?= 0
ifeq ($(VENDORED),1)
	ARGSV += --frozen
endif

REDOX ?= 0
ifeq ($(REDOX),1)
	undefine ARGSV
	ARGS += --target x86_64-unknown-redox
	TOOLCHAIN = nightly
endif

RUSTUP ?= 1
ifeq ($(RUSTUP),1)
	TOOLCHAIN_ARG = +$(TOOLCHAIN)
endif

.PHONY: tests all clean distclean install uninstall manual

all: $(SRC) $(GIT_REVISION)
ifeq ($(REDOX),1)
	mkdir -p .cargo
	grep redox .cargo/config || cat redox_linker >> .cargo/config
endif
ifeq ($(VENDORED),1)
	tar pxf vendor.tar.xz
endif
	cargo $(TOOLCHAIN_ARG) build $(ARGS) $(ARGSV)

manual:
	rm -rf manual/builtins
	mkdir manual/builtins
	cargo build --features man
	echo -n "# Builtin commands" > manual/src/builtins.md
	for man in manual/builtins/*; do \
		echo "" >> manual/src/builtins.md; \
		echo -n "## " >> manual/src/builtins.md; \
		cat $$man >> manual/src/builtins.md; \
	done

clean:
	cargo clean

distclean: clean
	rm -rf vendor vendor.tar.xz .cargo git_revision.txt

format:
	cargo +nightly fmt --all

tests:
	cargo $(TOOLCHAIN_ARG) test $(ARGSV)
	TOOLCHAIN=$(TOOLCHAIN) bash tests/run_examples.sh
	for crate in members/*; do \
		cargo $(TOOLCHAIN_ARG) test $(ARGSV) --manifest-path $$crate/Cargo.toml || exit 1; \
	done

test.%:
	TOOLCHAIN=$(TOOLCHAIN) bash tests/run_examples.sh $@

install:
	install -Dm0755 target/$(RELEASE)/ion $(DESTDIR)/$(BINARY)

uninstall:
	rm $(DESTDIR)/$(BINARY)

vendor: $(VENDOR)

version: $(GIT_REVISION)

$(GIT_REVISION):
	git rev-parse HEAD > git_revision.txt

$(VENDOR):
	rm -rf .cargo vendor vendor.tar.xz
	mkdir -p .cargo
	cargo vendor | head -n -1 > .cargo/config
	echo 'directory = "vendor"' >> .cargo/config
	tar pcfJ vendor.tar.xz vendor
	rm -rf vendor

update-shells:
	if ! grep ion /etc/shells >/dev/null; then \
		echo $(BINARY) >> /etc/shells; \
	else \
		shell=$(shell grep ion /etc/shells); \
		if [ $$shell != $(BINARY) ]; then \
			sed -i -e "s#$$shell#$(BINARY)#g" /etc/shells; \
		fi \
	fi

