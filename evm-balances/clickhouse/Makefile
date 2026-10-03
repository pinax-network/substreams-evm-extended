# Legacy SQL sink (substreams-sink-sql) targets from upstream substreams-evm
# `evm-balances/clickhouse`, with BSC defaults. The schema files are verbatim.
ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122288006
STOP_BLOCK ?= 122288007
PARALLEL_JOBS ?= 500
DSN ?= clickhouse://default:@localhost:9000/default
.DEFAULT_GOAL := pack

.PHONY: schema
schema:
	@echo "-- This file is generated. Do not edit." > schema.sql
	@echo "" >> schema.sql
	@for f in $$(ls schema.*.sql 2>/dev/null | sort); do \
		cat "$$f" >> schema.sql; \
		echo "" >> schema.sql; \
	done

.PHONY: pack
pack: schema
	substreams pack -o ../../spkg/{spkgDefaultName}

.PHONY: pack-norpc
pack-norpc: schema
	substreams pack substreams.norpc.yaml -o ../../spkg/{spkgDefaultName}

.PHONY: dev
dev: pack
	substreams-sink-sql run $(DSN) substreams.yaml -e $(ENDPOINT) $(START_BLOCK): --undo-buffer-size 0 --on-module-hash-mistmatch=warn --batch-block-flush-interval 1 --live-block-flush-interval 1 --infinite-retry --development-mode

.PHONY: setup
setup: pack
	substreams-sink-sql setup $(DSN) substreams.yaml
