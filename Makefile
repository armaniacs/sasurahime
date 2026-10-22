INSTALL_DIR ?= $(HOME)/.local/bin
BINARY      := sasurahime

VERSION := $(shell grep '^version' Cargo.toml | cut -d'"' -f2)
TAG     := v$(VERSION)

.PHONY: build test lint fmt install uninstall clean publish release release-tag \
        release-github tap-update check-deps outdated audit msrv

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

# Full release: gates, tag, GitHub release, crates.io, Homebrew tap.
release: lint test build release-tag release-github publish tap-update

release-tag:
	git tag $(TAG)
	git push origin $(TAG)

release-github:
	awk '/^## \[$(VERSION)\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md | \
		gh release create $(TAG) --repo armaniacs/sasurahime \
		--title "sasurahime $(TAG)" --notes-file -

tap-update:
	scripts/update-tap.sh $(TAG)

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
