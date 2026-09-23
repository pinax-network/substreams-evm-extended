SPKG := ../../spkg/erc4626-balance-state-v0.1.0.spkg
.DEFAULT_GOAL := build
.PHONY: build pack test
build:
	cargo build --locked -p erc4626-balance-state --target wasm32-unknown-unknown --release
# Live runs read SUBSTREAMS_API_KEY and RPC_URL from the environment only.
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p erc4626-balance-state
