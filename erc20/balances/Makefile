ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122260950
STOP_BLOCK ?= +64
SPKG := ../../spkg/erc20-balances-v0.2.0.spkg
.DEFAULT_GOAL := pack
.PHONY: build pack test run
build:
	cargo build --locked -p erc20-balances --target wasm32-unknown-unknown --release
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p erc20-balances
run: pack
	substreams run $(SPKG) map_events -e $(ENDPOINT) -s $(START_BLOCK) -t $(STOP_BLOCK) --final-blocks-only -o jsonl
