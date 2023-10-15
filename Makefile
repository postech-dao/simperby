.PHONY: all	build test clippy fmt

lint: fmt clippy 

fmt:
	cargo fmt

clippy:
	cargo clippy --all --all-targets --all-features -- -D warnings

test:
	cargo test --all --all-targets --all-features

build:
	cargo build

all: build test