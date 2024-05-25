.PHONY: all fmt check test regenerate

all: check

test:
	cargo nextest run

check: regenerate
	cargo check

regenerate: src/generated.rs

src/generated.rs: codegen/src/main.rs codegen/src/schema.rs codegen/Cargo.toml
	cd codegen ; cargo run
	cargo +nightly fmt

fmt:
	cd codegen ; cargo +nightly fmt
	cargo +nightly fmt
