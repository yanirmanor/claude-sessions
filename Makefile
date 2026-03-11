PREFIX ?= /usr/local/bin
BIN = claude-sessions

.PHONY: build install uninstall clean

build:
	cargo build --release

install: build
	cp target/release/$(BIN) $(PREFIX)/$(BIN)
	@echo "Installed $(BIN) to $(PREFIX)/$(BIN)"

uninstall:
	rm -f $(PREFIX)/$(BIN)
	@echo "Removed $(BIN) from $(PREFIX)/$(BIN)"

clean:
	cargo clean
