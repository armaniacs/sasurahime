INSTALL_DIR ?= $(HOME)/.local/bin
BINARY      := sasurahime

.PHONY: build test lint fmt install clean

build:
	cargo build --release

test:
	cargo test

lint:
	cargo fmt --check
	cargo clippy --tests -- -D warnings

fmt:
	cargo fmt

install: build
	@mkdir -p $(INSTALL_DIR)
	cp target/release/$(BINARY) $(INSTALL_DIR)/$(BINARY)
	@echo "Installed to $(INSTALL_DIR)/$(BINARY)"

uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY)
	@echo "Removed $(INSTALL_DIR)/$(BINARY)"

clean:
	cargo clean
