.PHONY: dev build run release

dev:
	cargo run --manifest-path crates/ephemera-app/Cargo.toml

build:
	cargo build --release --manifest-path crates/ephemera-app/Cargo.toml

run:
	./crates/ephemera-app/target/release/ephemera-app

release: build run
