SPKG := ../../spkg/aave-balance-state-v0.1.0.spkg
.DEFAULT_GOAL := build
.PHONY: build pack test
build:
	cargo build --locked -p aave-balance-state --target wasm32-unknown-unknown --release
# Packaging and live streaming stay paused until explicitly resumed.
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p aave-balance-state
