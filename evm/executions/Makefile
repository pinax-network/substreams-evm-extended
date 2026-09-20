SPKG := ../../spkg/evm-executions-v0.1.0.spkg
.DEFAULT_GOAL := build
.PHONY: build pack test
build:
	cargo build --locked -p evm-executions --target wasm32-unknown-unknown --release
# Packaging and live streaming stay paused until explicitly resumed.
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p evm-executions
