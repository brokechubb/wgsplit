.PHONY: all build install clean

PREFIX ?= /usr/local

all: build

build:
	cargo build --release
	cd tui && bun install && bun build src/index.tsx --compile --outfile wgsplit

install:
	install -Dm755 target/release/wgsplitd $(DESTDIR)$(PREFIX)/bin/wgsplitd
	install -Dm755 tui/wgsplit $(DESTDIR)$(PREFIX)/bin/wgsplit
	install -Dm644 contrib/wgsplitd.service $(DESTDIR)/etc/systemd/system/wgsplitd.service

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/wgsplitd
	rm -f $(DESTDIR)$(PREFIX)/bin/wgsplit
	rm -f $(DESTDIR)/etc/systemd/system/wgsplitd.service

clean:
	cargo clean
	rm -f tui/wgsplit tui/wgsplit-tui
