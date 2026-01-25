# Makefile for rookie project
# A Rust implementation of DWDS rule-taker (RT) and rule-reserve (RR) components

# Configuration
CARGO := cargo
INSTALL_DIR := /usr/local/bin
DATA_DIR := ./data
TEST_DIR := ./test

# Binaries
BINARIES := api ingest invoke select sync daemon parse

# Default target
.PHONY: all
all: build

# Build all binaries
.PHONY: build
build:
	@echo "Building all binaries..."
	$(CARGO) build --release

# Build specific binary
.PHONY: build-debug
build-debug:
	@echo "Building debug binaries..."
	$(CARGO) build

# Build individual binaries (for selective compilation)
.PHONY: build-api build-ingest build-invoke build-select build-sync
build-api:
	$(CARGO) build --release --bin api

build-ingest:
	$(CARGO) build --release --bin ingest

build-invoke:
	$(CARGO) build --release --bin invoke

build-select:
	$(CARGO) build --release --bin select

build-sync:
	$(CARGO) build --release --bin sync

# Install binaries to system
.PHONY: install
install: build
	@echo "Installing binaries to $(INSTALL_DIR)..."
	@mkdir -p $(INSTALL_DIR)
	@for bin in $(BINARIES); do \
		if [ -f target/release/$$bin ]; then \
			echo "Installing $$bin..."; \
			install -m 755 target/release/$$bin $(INSTALL_DIR)/; \
		fi \
	done
	@echo "Installation complete!"

# Run Rust unit tests
.PHONY: test
test: test-unit test-integration

# Run unit tests only
.PHONY: test-unit
test-unit:
	@echo "Running Rust unit tests..."
	$(CARGO) test

BATS_OPTS=
ifdef VERBOSE
	BATS_OPTS+=--verbose-run
endif
BATS_TESTS_DIR=test/bats
# Run integration tests (BATS)
.PHONY: test-integration
test-integration: build-debug
	@echo "Running BATS integration tests..."
	bats $(BATS_OPTS) -r $(BATS_TESTS_DIR); \

# Run all tests with coverage (requires cargo-tarpaulin)
.PHONY: test-coverage
test-coverage:
	@echo "Running tests with coverage..."
	@if command -v cargo-tarpaulin >/dev/null 2>&1; then \
		cargo tarpaulin --out Html --output-dir coverage; \
		echo "Coverage report generated in coverage/"; \
	else \
		echo "ERROR: cargo-tarpaulin not found. Install with 'cargo install cargo-tarpaulin'"; \
		exit 1; \
	fi

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	$(CARGO) clean
	@echo "Build artifacts cleaned"

# Clean everything including data
.PHONY: clean-all
clean-all: clean
	@echo "Cleaning data directory..."
	rm -rf $(DATA_DIR)/rules.db
	rm -rf $(DATA_DIR)/rules/*
	@echo "All artifacts and data cleaned"

# Format code
.PHONY: format
format:
	@echo "Formatting code..."
	$(CARGO) fmt

# Check code formatting
.PHONY: fmt-check
fmt-check:
	@echo "Checking code formatting..."
	$(CARGO) fmt -- --check

# Run clippy linter
.PHONY: lint
lint:
	@echo "Running clippy..."
	$(CARGO) clippy -- -D warnings

# Run clippy with fixes
.PHONY: lint-fix
lint-fix:
	@echo "Running clippy with automatic fixes..."
	$(CARGO) clippy --fix --allow-dirty --allow-staged

.PHONY: cargo-check
cargo-check:
	$(CARGO) check

# Check project (fast compile check without building)
.PHONY: check
check: cargo-check lint fmt-check
	@echo "Checking project..."

# Run the API server (development)
.PHONY: run-api
run-api: build-debug
	@echo "Starting API server..."
	$(CARGO) run --bin api

# Initialize data directories
.PHONY: init
init:
	@echo "Initializing data directories..."
	@mkdir -p $(DATA_DIR)/rules
	@mkdir -p etc/contexts
	@if [ ! -f etc/contexts/default.json ]; then \
		echo '{"jurisdiction": "US-CA", "tz": "America/Los_Angeles"}' > etc/contexts/default.json; \
		echo "Created default context file"; \
	fi
	@echo "Initialization complete"

# Development setup
.PHONY: dev-setup
dev-setup: init
	@echo "Setting up development environment..."
	@if command -v rustup >/dev/null 2>&1; then \
		rustup component add rustfmt clippy; \
		echo "Rust components installed"; \
	fi
	@echo "Development setup complete"

# Create release build with optimizations
.PHONY: release
release: test lint build
	@echo "Creating release build..."
	@echo "All checks passed, binaries ready in target/release/"

# Quick development cycle
.PHONY: dev
dev: format build-debug test-unit
	@echo "Development cycle complete"

# CI target (what CI should run)
.PHONY: ci
ci: fmt-check lint check test
	@echo "CI checks passed"

# Watch for changes and rebuild (requires cargo-watch)
.PHONY: watch
watch:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		cargo watch -x build; \
	else \
		echo "ERROR: cargo-watch not found. Install with 'cargo install cargo-watch'"; \
		exit 1; \
	fi

# Generate documentation
.PHONY: doc
doc:
	@echo "Generating documentation..."
	$(CARGO) doc --no-deps --open
