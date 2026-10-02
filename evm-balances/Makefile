# Legacy db_out patch: upstream substreams-evm `evm-balances` targets, BSC defaults.
ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122288006
STOP_BLOCK ?= +1000
PARALLEL_JOBS ?= 500
.DEFAULT_GOAL := pack

.PHONY: build
build:
	cargo build --locked -p db-evm-balances --target wasm32-unknown-unknown --release

.PHONY: pack
pack: build
	substreams pack -o ../spkg/{spkgDefaultName}

# Offline: compare the committed package with the deployed substreams-evm v0.3.4.
.PHONY: compare
compare:
	cd .. && cargo run --locked -p evm-balances-tools -- \
	  --built-wasm target/wasm32-unknown-unknown/release/db_evm_balances.wasm --output $(OUTPUT)

.PHONY: noop
noop: build
	substreams-sink-noop $(ENDPOINT) substreams.yaml db_out -H "X-Sf-Substreams-Parallel-Jobs: $(PARALLEL_JOBS)" $(START_BLOCK):$(STOP_BLOCK)

.PHONY: gui
gui: build
	substreams gui -e $(ENDPOINT) substreams.yaml db_out -s $(START_BLOCK) --network bsc

.PHONY: prod
prod: build
	substreams gui -e $(ENDPOINT) substreams.yaml db_out -s $(START_BLOCK) -t 0 --limit-processed-blocks 0 --production-mode  -H "X-Sf-Substreams-Parallel-Jobs: $(PARALLEL_JOBS)"
