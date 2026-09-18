# Run from this directory. The native CLI owns schema creation and ingestion.
REPO := ../../..
SUBSTREAMS ?= substreams
PACKAGE ?= $(REPO)/spkg/erc20-balances-v0.1.0.spkg
LAYOUTS ?= ../tests/fixtures/bridge450/layouts.json
STATE ?= $(CURDIR)/localdata/bsc-balances
ENDPOINT ?= bsc.substreams.pinax.network:443
START ?= 122288006
STOP ?= 122288150
SMOKE_OUTPUT ?= erc20/balances/out/native-clickhouse-smoke

.PHONY: setup run smoke
setup:
	@test -n "$$SUBSTREAMS_SINK_DSN" || (echo 'Set SUBSTREAMS_SINK_DSN to an isolated ClickHouse database' >&2; exit 1)
	mkdir -p "$(STATE)/meta"
	$(SUBSTREAMS) sink clickhouse setup "$(PACKAGE)" map_events --bytes-encoding 0xhex --sink-info-folder "$(STATE)/meta"

run:
	@test -n "$$SUBSTREAMS_SINK_DSN" || (echo 'Set SUBSTREAMS_SINK_DSN' >&2; exit 1)
	$(SUBSTREAMS) sink clickhouse "$(PACKAGE)" map_events -e "$(ENDPOINT)" -s "$(START)" -t "$(STOP)" \
	  -p "map_events=$$(cat "$(LAYOUTS)")" --final-blocks-only --bytes-encoding 0xhex \
	  --sink-info-folder "$(STATE)/meta" --cursor-file-path "$(STATE)/cursor.txt" --spool-dir "$(STATE)/spool" \
	  --spool-max-size 1GiB --spool-max-idle 100ms --decode-batch-size 1

smoke:
	cd "$(REPO)" && cargo run --locked -p erc20-balances-tools --example native_clickhouse_smoke -- \
	  --package "$(abspath $(PACKAGE))" --substreams "$(SUBSTREAMS)" --endpoint "$(ENDPOINT)" --output "$(SMOKE_OUTPUT)"
