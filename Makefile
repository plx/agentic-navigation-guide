# Makefile for agentic-navigation-guide development

.PHONY: all build check test lint fmt clippy verify clean install-hooks

# Default target
all: check

# Build the project
build:
	cargo build

# Quick check without building
check:
	cargo check

# Run all tests
test:
	cargo test

# Run all linting checks (same as pre-commit hook)
lint: fmt-check clippy

# Check formatting
fmt-check:
	cargo fmt -- --check

# Format code
fmt:
	cargo fmt

# Run clippy with warnings as errors
clippy:
	cargo clippy -- -D warnings

# Verify navigation guide
verify:
	cargo run -- verify

# Clean build artifacts
clean:
	cargo clean

# Install pre-commit hook
install-hooks:
	@echo "Installing pre-commit hook..."
	@cp scripts/pre-commit .git/hooks/pre-commit 2>/dev/null || \
		(gitdir=$$(cat .git 2>/dev/null | sed 's/gitdir: //'); \
		 if [ -n "$$gitdir" ]; then \
			cp scripts/pre-commit "$$gitdir/hooks/pre-commit"; \
		 else \
			echo "Could not find git hooks directory"; \
			exit 1; \
		 fi)
	@chmod +x .git/hooks/pre-commit 2>/dev/null || true
	@echo "Pre-commit hook installed!"

# Run all CI checks locally
ci: lint test verify
	@echo "All CI checks passed!"
