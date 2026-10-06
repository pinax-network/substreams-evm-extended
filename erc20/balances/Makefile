ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122288006
STOP_BLOCK ?= +1000
PARALLEL_JOBS ?= 500
.DEFAULT_GOAL := pack

.PHONY: build
# Reproducible WASM paths and hashes: see build.mk at the repository root.
include ../../build.mk
build: export RUSTFLAGS := $(WASM_RUSTFLAGS)
build:
	cargo build --locked --target wasm32-unknown-unknown --release

.PHONY: pack
pack: build
	substreams pack -o ../../spkg/{spkgDefaultName}

.PHONY: gui
gui: build
	substreams gui -e $(ENDPOINT) substreams.yaml map_events -s $(START_BLOCK) --limit-processed-blocks 0

.PHONY: prod
prod: build
	substreams gui -e $(ENDPOINT) substreams.yaml map_events -s $(START_BLOCK) -t $(STOP_BLOCK) --limit-processed-blocks 0 --production-mode  -H "X-Substreams-Parallel-Workers: $(PARALLEL_JOBS)"
