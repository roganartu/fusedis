build:
	cargo build

build-release:
	cargo build --release

format:
	rustfmt -l src/main.rs

lint:
	cargo clippy --all-features --all --tests --examples -- -D clippy::all -D warnings
