prefix ?= usr/local
BINARY = $(prefix)/bin/ion
RELEASE = debug
DEBUG ?= 0
VENDORED = 0
REDOX ?= 0
TOOLCHAIN ?= 1.31.0

GIT_REVISION=git_revision.txt
SRC=Cargo.toml src/* src/*/* members/* members/*/*
VENDOR=.cargo/config vendor.tar.xz

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
	TOOLCHAIN = nightly
endif

.PHONY: all clean distclean install uninstall

all: $(SRC) $(GIT_REVISION)
ifeq (1,$(REDOX))
	mkdir -p .cargo
	grep redox .cargo/config || cat redox_linker >> .cargo/config
endif
	cargo +$(TOOLCHAIN) build $(ARGS) $(ARGSV)

clean:
	cargo clean

distclean: clean
	rm -rf vendor vendor.tar.xz .cargo git_revision.txt

tests:
	cargo +$(TOOLCHAIN) test $(ARGSV)
	TOOLCHAIN=$(TOOLCHAIN) bash examples/run_examples.sh
	for crate in members/*; do \
		cargo +$(TOOLCHAIN) test $(ARGSV) --manifest-path $$crate/Cargo.toml || exit 1; \
	done

install:
	install -Dm0755 target/$(RELEASE)/ion $(DESTDIR)/$(BINARY)

uninstall:
	rm $(DESTDIR)/$(BINARY)

vendor: $(VENDOR)

version: $(GIT_REVISION)

$(GIT_REVISION):
	git rev-parse master > git_revision.txt

$(VENDOR):
	mkdir -p .cargo
	cargo +$(TOOLCHAIN) vendor | head -n -1 > .cargo/config
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

