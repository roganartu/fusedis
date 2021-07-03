build:
	cargo build

build-release:
	cargo build --release

format:
	rustfmt -l src/**

lint:
	cargo clippy --all-features --all --tests --examples -- -D clippy::all -D warnings
