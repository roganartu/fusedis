build:
	cargo build

build-release:
	cargo build --release

format:
	rustfmt -l src/**
