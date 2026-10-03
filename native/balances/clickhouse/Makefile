# Native balances into ClickHouse, two ways from the same package:
# * native sink (`setup`, `run`): the Substreams CLI's ClickHouse sink loads
#   map_events into `Balance`/`_blocks_`; schema.4 attaches the legacy
#   substreams-evm native tables and views, schema.5 sets the base-data TTL.
# * legacy sink (`legacy-setup`, `legacy-run`): substreams-sink-sql runs db_out
#   into the same legacy tables (schema.0–3, embedded in the package).
# SUBSTREAMS_SINK_DSN (clickhouse://user:password@host:9000/db) selects the
# database; use one database per path. It is never printed.
REPO := ../../..
SUBSTREAMS ?= substreams
CLICKHOUSE ?= clickhouse
SINK_SQL ?= substreams-sink-sql
PACKAGE ?= $(REPO)/spkg/native-balances-v0.2.0.spkg
ENDPOINT ?= bsc.substreams.pinax.network:443
TTL_DAYS ?= 7
STATE ?= $(CURDIR)/localdata/bsc-native-balances
START ?=
STOP ?= 0
# Empty runs the legacy sink without a stop block.
LEGACY_STOP ?=

.PHONY: setup sink-setup schema run legacy-setup legacy-run check-dsn

check-dsn:
	@test -n "$$SUBSTREAMS_SINK_DSN" || (echo 'Set SUBSTREAMS_SINK_DSN to the target ClickHouse database' >&2; exit 1)

# Order matters: the views must exist before the sink's first insert.
setup: sink-setup schema

sink-setup: check-dsn
	mkdir -p "$(STATE)/meta"
	$(SUBSTREAMS) sink clickhouse setup "$(PACKAGE)" map_events --bytes-encoding 0xhex --sink-info-folder "$(STATE)/meta"

schema: check-dsn
	@for f in $$(ls schema.*.sql | sort); do \
		echo "applying $$f"; \
		$(CLICKHOUSE) client "$$SUBSTREAMS_SINK_DSN" --param_ttl_days=$(TTL_DAYS) --multiquery --queries-file "$$f" || exit 1; \
	done

run: check-dsn
	@test -n "$(START)" || (echo 'Set START to a producer-version-5 block' >&2; exit 1)
	$(SUBSTREAMS) sink clickhouse "$(PACKAGE)" map_events -e "$(ENDPOINT)" -s "$(START)" -t "$(STOP)" \
	  --final-blocks-only --bytes-encoding 0xhex \
	  --sink-info-folder "$(STATE)/meta" --cursor-file-path "$(STATE)/cursor.txt" --spool-dir "$(STATE)/spool"

# substreams-sink-sql expects the database to exist; create it through the
# server's `default` database, keeping any `?options` of the DSN.
legacy-setup: check-dsn
	@rest="$${SUBSTREAMS_SINK_DSN##*/}"; db="$${rest%%\?*}"; \
	  $(CLICKHOUSE) client "$${SUBSTREAMS_SINK_DSN%/*}/default$${rest#"$$db"}" --query "CREATE DATABASE IF NOT EXISTS \`$$db\`" < /dev/null
	$(SINK_SQL) setup "$$SUBSTREAMS_SINK_DSN" "$(PACKAGE)"

legacy-run: check-dsn
	@test -n "$(START)" || (echo 'Set START to a producer-version-5 block' >&2; exit 1)
	$(SINK_SQL) run "$$SUBSTREAMS_SINK_DSN" "$(PACKAGE)" -e "$(ENDPOINT)" "$(START):$(LEGACY_STOP)" \
	  --final-blocks-only --on-module-hash-mistmatch=warn

