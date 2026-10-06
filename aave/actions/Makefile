SPKG := ../../spkg/aave-actions-v0.1.0.spkg
.DEFAULT_GOAL := build
.PHONY: build pack test
# Reproducible WASM paths and hashes: see build.mk at the repository root.
include ../../build.mk
build: export RUSTFLAGS := $(WASM_RUSTFLAGS)
build:
	cargo build --locked -p aave-actions --target wasm32-unknown-unknown --release
# Live runs read SUBSTREAMS_API_KEY and RPC_URL from the environment only.
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p aave-actions
