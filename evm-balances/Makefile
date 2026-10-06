ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122288006
STOP_BLOCK ?= +1000
PARALLEL_JOBS ?= 500
# ClickHouse DSN for the SQL sink, read from the environment and passed quoted,
# so it is never echoed. setup does not create the database.
export SUBSTREAMS_SINK_DSN ?= clickhouse://default:@localhost:9000/default
.DEFAULT_GOAL := pack

# substreams pack embeds the imported packages' WASM without building it.
# One cargo build per package: built together, substreams-database-change
# turns on buffa's json feature for the imported maps too, so their WASM and
# module hashes would differ from the native/balances and erc20/balances
# packages'.
# Reproducible WASM paths and hashes: see build.mk at the repository root.
include ../build.mk
build: export RUSTFLAGS := $(WASM_RUSTFLAGS)
.PHONY: build
build:
	cargo build --locked --target wasm32-unknown-unknown --release -p native-balances
	cargo build --locked --target wasm32-unknown-unknown --release -p erc20-balances
	cargo build --locked --target wasm32-unknown-unknown --release -p evm-balances

.PHONY: schema
schema:
	@echo "-- This file is generated. Do not edit." > clickhouse/schema.sql
	@echo "" >> clickhouse/schema.sql
	@for f in $$(ls clickhouse/schema.*.sql 2>/dev/null | sort); do \
		cat "$$f" >> clickhouse/schema.sql; \
		echo "" >> clickhouse/schema.sql; \
	done

.PHONY: pack
pack: build schema
	substreams pack -o ../spkg/{spkgDefaultName}

.PHONY: gui
gui: build schema
	substreams gui -e $(ENDPOINT) substreams.yaml db_out -s $(START_BLOCK) --limit-processed-blocks 0

.PHONY: prod
prod: build schema
	substreams gui -e $(ENDPOINT) substreams.yaml db_out -s $(START_BLOCK) -t $(STOP_BLOCK) --limit-processed-blocks 0 --production-mode  -H "X-Substreams-Parallel-Workers: $(PARALLEL_JOBS)"

.PHONY: noop
noop: build schema
	substreams-sink-noop $(ENDPOINT) substreams.yaml db_out -H "X-Substreams-Parallel-Workers: $(PARALLEL_JOBS)" $(START_BLOCK):$(STOP_BLOCK)

.PHONY: setup
setup: build schema
	substreams-sink-sql setup "$$SUBSTREAMS_SINK_DSN" substreams.yaml

.PHONY: dev
dev: build schema
	substreams-sink-sql run "$$SUBSTREAMS_SINK_DSN" substreams.yaml -e $(ENDPOINT) $(START_BLOCK): --final-blocks-only --on-module-hash-mistmatch=warn --batch-block-flush-interval 1 --live-block-flush-interval 1 --infinite-retry --development-mode
