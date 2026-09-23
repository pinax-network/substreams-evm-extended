ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122260950
STOP_BLOCK ?= +64
PARAMS ?= {"producer_versions":[5]}
SPKG := ../../spkg/native-balances-v0.1.0.spkg
.DEFAULT_GOAL := pack
.PHONY: build pack test run replay
build:
	cargo build --locked -p native-balances --target wasm32-unknown-unknown --release
pack: build
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p native-balances -p native-balances-tools -p evm-persist
# Offline replay over captured Extended block files; no network access.
BLOCKS ?= ../../erc20/balances/out/top50-full-holder-blocks
OUTPUT ?= out/replay
replay:
	cargo run --locked -p native-balances-tools -- replay --blocks $(BLOCKS) --output $(OUTPUT) \
	  --oracle ../../erc20/balances/tests/fixtures/bsc-122260950.json \
	  --prototype-rows tests/fixtures/bsc-122260950-prototype-native-rows.json
# Live streaming reads SUBSTREAMS_API_KEY from the environment.
run: pack
	substreams run $(SPKG) map_events -p 'map_events=$(PARAMS)' -e $(ENDPOINT) -s $(START_BLOCK) -t $(STOP_BLOCK) --final-blocks-only -o jsonl
