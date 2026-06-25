.PHONY: help fmt lint test audit build clean install dev

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

fmt: ## Format all Rust code
	cd project && cargo fmt --all

lint: ## Run clippy on all targets
	cd project && cargo clippy --all-targets --all-features -- -D warnings

test: ## Run all tests
	cd project && cargo test --all --all-features --verbose

audit: ## Run security audit
	cd project && cargo audit && cargo deny check

coverage: ## Generate test coverage report
	cd project && cargo tarpaulin --out Html --all --all-features

build: ## Build release binary
	cd project && cargo build --release

clean: ## Clean build artifacts
	cd project && cargo clean
	cd project && rm -rf node_modules dist

install: ## Install development dependencies
	cd project && npm ci
	cargo install cargo-audit cargo-deny cargo-tarpaulin cargo-nextest

dev: ## Start development server
	cd project && npm run dev
