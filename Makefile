PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin

build:
	cargo build --release

install:
	install -Dm755 target/release/meteobar $(DESTDIR)$(BINDIR)/meteobar

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/meteobar

.PHONY: build install uninstall
