.PHONY: all fmt check test regenerate

all: check

test:
	cargo nextest run

check: regenerate
	cargo check

regenerate: codegen/src/main.rs
	cd codegen ; cargo run
	cargo +nightly fmt

fmt:
	cd codegen ; cargo +nightly fmt
	cargo +nightly fmt
