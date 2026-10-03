ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122260950
STOP_BLOCK ?= +64
PARAMS ?= {"producer_versions":[5]}
# Local builds stay under out/. spkg/native-balances-v0.2.0.spkg is the committed
# deploy package (`make pack SPKG=../../spkg/native-balances-v0.2.0.spkg`), and
# spkg/native-balances-v0.1.0.spkg the live-qualified map-only build (fbb46fc7…).
SPKG ?= out/native-balances-v0.2.0.spkg
.DEFAULT_GOAL := pack
.PHONY: build schema pack test run replay
build:
	cargo build --locked -p native-balances -p native-balances-db-out --target wasm32-unknown-unknown --release
# The legacy SQL sink's schema: the four verbatim substreams-evm native files,
# concatenated as upstream's `make schema` does. The native sink path applies
# all of clickhouse/schema.*.sql instead (clickhouse/README.md).
schema:
	@echo "-- This file is generated. Do not edit." > clickhouse/schema.sql
	@echo "" >> clickhouse/schema.sql
	@for f in clickhouse/schema.0.blocks.sql clickhouse/schema.1.table.native-balances.sql \
	  clickhouse/schema.2.mv.historical-native-balances.sql clickhouse/schema.3.view.historical-native-balances.sql; do \
		cat "$$f" >> clickhouse/schema.sql; \
		echo "" >> clickhouse/schema.sql; \
	done
pack: build schema
	@mkdir -p $(dir $(SPKG))
	substreams pack substreams.yaml -o $(SPKG)
test:
	cargo test --locked -p native-balances -p native-balances-db-out -p native-balances-tools -p evm-persist
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
