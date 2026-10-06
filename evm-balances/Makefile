ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122288006
STOP_BLOCK ?= +1000
PARALLEL_JOBS ?= 500
# ClickHouse DSN for the SQL sink, read from the environment and passed quoted,
# so it is never echoed. setup does not create the database.
export SUBSTREAMS_SINK_DSN ?= clickhouse://default:@localhost:9000/default
# The supported sink (#120): pinax-network/substreams-sink-sql at this tag and
# commit. `install-sink` builds it; `setup` and `dev` refuse any other build.
SINK_VERSION := v4.13.0
SINK_COMMIT := 933a187
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

# The fork's go.mod keeps the upstream module path, so `go install <path>@<tag>`
# cannot fetch it: build from a clone of the tag instead.
.PHONY: install-sink
install-sink:
	tmp=$$(mktemp -d) && \
	git clone --quiet --depth 1 --branch $(SINK_VERSION) https://github.com/pinax-network/substreams-sink-sql "$$tmp" && \
	(cd "$$tmp" && go install -ldflags "-X main.version=$(SINK_VERSION)" ./cmd/substreams-sink-sql) && \
	rm -rf "$$tmp"
	substreams-sink-sql --version

.PHONY: check-sink
check-sink:
	@substreams-sink-sql --version | grep -q "(Commit $(SINK_COMMIT)" || \
		{ echo "substreams-sink-sql is not $(SINK_VERSION) ($(SINK_COMMIT)); run make install-sink" >&2; exit 1; }

.PHONY: setup
setup: build schema check-sink
	substreams-sink-sql setup "$$SUBSTREAMS_SINK_DSN" substreams.yaml

.PHONY: dev
dev: build schema check-sink
	substreams-sink-sql run "$$SUBSTREAMS_SINK_DSN" substreams.yaml -e $(ENDPOINT) $(START_BLOCK): --final-blocks-only --on-module-hash-mistmatch=warn --batch-block-flush-interval 1 --live-block-flush-interval 1 --infinite-retry --development-mode
