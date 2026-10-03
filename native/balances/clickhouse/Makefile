# Native balances into ClickHouse with the Substreams CLI's native sink (no
# db_out). The sink loads map_events into `Balance` and `_blocks_`; schema.1–3
# add the legacy substreams-evm native tables and views, schema.4 feeds them
# from `Balance`, schema.5 sets a 7-day base-data TTL. `_blocks_` is the block table.
ENDPOINT ?= bsc.substreams.pinax.network:443
START_BLOCK ?= 122288006
STOP_BLOCK ?= +100
MANIFEST ?= ../substreams.yaml
STATE ?= localdata
# Read by the sink and `clickhouse client` from the environment, so a password
# is never echoed.
export SUBSTREAMS_SINK_DSN ?= clickhouse://default:@localhost:9000/default
.DEFAULT_GOAL := schema

.PHONY: schema
schema:
	@echo "-- This file is generated. Do not edit." > schema.sql
	@echo "" >> schema.sql
	@for f in $$(ls schema.*.sql 2>/dev/null | sort); do \
		cat "$$f" >> schema.sql; \
		echo "" >> schema.sql; \
	done

# The local manifest references the release WASM.
.PHONY: build
build:
	$(MAKE) -C .. build

# The sink's tables first, then schema.sql: the view must exist before the
# sink's first insert.
.PHONY: setup
setup: build schema
	@mkdir -p $(STATE)/meta
	substreams sink clickhouse setup $(MANIFEST) map_events --bytes-encoding 0xhex --sink-info-folder $(STATE)/meta
	clickhouse client "$$SUBSTREAMS_SINK_DSN" --multiquery --queries-file schema.sql

# STOP_BLOCK=0 streams without a stop block.
.PHONY: dev
dev: build
	substreams sink clickhouse $(MANIFEST) map_events -e $(ENDPOINT) -s $(START_BLOCK) -t $(STOP_BLOCK) \
	  --final-blocks-only --bytes-encoding 0xhex \
	  --sink-info-folder $(STATE)/meta --cursor-file-path $(STATE)/cursor.txt --spool-dir $(STATE)/spool
