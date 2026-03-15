PREFIX ?= /usr/local/bin
BIN = ai-sessions

.PHONY: build install uninstall clean

build:
	cargo build --release

install:
	cargo install --path .
	@echo "Installed $(BIN)"

uninstall:
	rm -f $(PREFIX)/$(BIN)
	@echo "Removed $(BIN) from $(PREFIX)/$(BIN)"

update:
	git pull
	$(MAKE) install

clean:
	cargo clean
