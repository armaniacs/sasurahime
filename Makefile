INSTALL_DIR ?= $(HOME)/.local/bin
BINARY      := sasurahime

.PHONY: build test lint fmt install uninstall clean publish release \
        check-deps outdated audit msrv

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

publish:
	cargo publish

release: lint test build publish

# ── Dependency checks ────────────────────────────────────────────────────

check-deps: outdated audit
	@echo "✅ All dependency checks passed"

outdated:
	@echo "=== Checking outdated dependencies ==="
	@if command -v cargo-outdated >/dev/null 2>&1; then \
		cargo outdated --root-deps-only; \
	else \
		echo "⚠️   cargo-outdated not installed."; \
		echo "    Install: cargo install cargo-outdated"; \
	fi

audit:
	@echo "=== Checking security advisories ==="
	@if command -v cargo-audit >/dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "⚠️   cargo-audit not installed."; \
		echo "    Install: cargo install cargo-audit"; \
	fi

msrv:
	@echo "=== Checking MSRV compatibility ==="
	@if command -v cargo-msrv >/dev/null 2>&1; then \
		cargo msrv; \
	else \
		echo "⚠️   cargo-msrv not installed."; \
		echo "    Install: cargo install cargo-msrv"; \
	fi
