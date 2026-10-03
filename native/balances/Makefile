ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122260950
STOP_BLOCK ?= +64
PARAMS ?= {"producer_versions":[5]}
SPKG := ../../spkg/native-balances-v0.1.0.spkg
.DEFAULT_GOAL := pack
.PHONY: build pack test run
build:
	cargo build --locked -p native-balances --target wasm32-unknown-unknown --release
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p native-balances -p evm-persist
# Live streaming reads SUBSTREAMS_API_KEY from the environment.
run: pack
	substreams run $(SPKG) map_events -p 'map_events=$(PARAMS)' -e $(ENDPOINT) -s $(START_BLOCK) -t $(STOP_BLOCK) --final-blocks-only -o jsonl
