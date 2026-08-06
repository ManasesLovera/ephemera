.PHONY: dev build run release

dev:
	pnpm tauri dev

build:
	pnpm tauri build

run:
	./src-tauri/target/release/ephemera

release: build run
