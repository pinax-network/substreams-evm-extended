#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    offline::main()
}

#[cfg(not(target_arch = "wasm32"))]
mod offline {
    use anyhow::{ensure, Context, Result};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use erc20_balances_tools::{bnbtiger_cookie_candidates as bound, bnbtiger_cookie_proof::Target};
    use evm_retention::{Clock, Domain, Key, Ledger, Lookup};
    use prost::Message;
    use proto::pb::evm::balances::v1::{Balance, Events};
    use serde_json::{json, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{self, File},
        io::{BufRead, BufReader, BufWriter, Write},
        path::Path,
    };
    use substreams_ethereum::pb::eth::v2 as eth;
    const START: u64 = 122288006;
    const STOP: u64 = 122289030;
    type Counts = BTreeMap<String, BTreeMap<String, u64>>;
    const CACHE_PINS: [(&str, &str); 7] = [
        (
            "out/refined450-combined/events.jsonl",
            "45aae8828305219b39952312612ec4a6ed713383e9e82bbf3fbab66ba329ee9d",
        ),
        (
            "out/refined450-combined/report.json",
            "cf4b38f2a2c7793e5bbd1be47cea5a35fd50ee574755a13c2fa16b91594fda70",
        ),
        (
            "out/refined450-combined/events.clocks.txt",
            "2a282311d129f5e4f1d9bdf31888ab2585666e64d54313fb462616f70c339c10",
        ),
        (
            "out/top50-1024/reference.jsonl",
            "ebfff9a998618b3e7e9f7802d20d49c9890f7295de189315393e636a65fc88d9",
        ),
        ("out/top50-1024/report.json", "be1c62a18ee750c2f2f824272566bca9d69942c67f794e88e80f57b34df892c5"),
        (
            "out/ranks401-450-survey/report.json",
            "0cde910d1982bc418760e9ec718797151e8a5529d550992c4ab11ad44edcdf7b",
        ),
        (
            "out/reflection-replay/blocks.json",
            "56397f18ea7371231b261baefb10122c1439e604d130be2cbc97fca598e38ba6",
        ),
    ];
    fn block_manifest(value: &Value, clocks: &BTreeMap<u64, String>) -> Result<BTreeMap<u64, String>> {
        let rows = value.as_array().context("original PB manifest")?;
        ensure!(rows.len() == 1024 && clocks.keys().copied().eq(START..STOP), "PB manifest/clock completeness");
        let mut out = BTreeMap::new();
        for (h, row) in (START..STOP).zip(rows) {
            ensure!(
                row.as_object()
                    .context("PB manifest row")?
                    .keys()
                    .all(|k| ["block", "hash", "sha256"].contains(&k.as_str()))
                    && row["block"] == h
                    && row["hash"] == clocks[&h],
                "PB manifest order/height/hash"
            );
            let digest = row["sha256"].as_str().context("PB digest")?;
            ensure!(digest.len() == 64 && hex::decode(digest)?.len() == 32, "PB digest width");
            out.insert(h, digest.to_owned());
        }
        Ok(out)
    }
    fn verify_block_bytes(bytes: &[u8], height: u64, pins: &BTreeMap<u64, String>) -> Result<()> {
        ensure!(
            pins.get(&height) == Some(&bound::sha(bytes)),
            "saved PB differs from immutable original manifest at {height}"
        );
        Ok(())
    }
    fn read(path: &Path) -> Result<Value> {
        bound::read(path)
    }
    fn binary(v: &Value, size: usize) -> Result<Vec<u8>> {
        let s = v.as_str().context("bytes")?;
        let raw = if let Some(h) = s.strip_prefix("0x") {
            hex::decode(h)?
        } else {
            STANDARD.decode(s)?
        };
        ensure!(raw.len() == size, "binary width");
        Ok(raw)
    }
    fn normalize(v: &mut Events) -> Result<()> {
        v.balances.sort_by(|a, b| (&a.contract, &a.address).cmp(&(&b.contract, &b.address)));
        ensure!(
            v.balances
                .windows(2)
                .all(|w| (&w[0].contract, &w[0].address) != (&w[1].contract, &w[1].address)),
            "duplicate balance key"
        );
        Ok(())
    }
    fn events(v: &Value) -> Result<Events> {
        let obj = v.as_object().context("Events object")?;
        ensure!(obj.keys().all(|k| k == "balances"), "unknown Events field");
        let mut out = Events::default();
        if let Some(rows) = obj.get("balances") {
            for row in rows.as_array().context("rows")? {
                ensure!(
                    row.as_object()
                        .context("row")?
                        .keys()
                        .all(|k| ["contract", "address", "amount"].contains(&k.as_str())),
                    "unknown Balance field"
                );
                let amount = row["amount"].as_str().context("amount")?;
                ensure!(
                    !amount.is_empty() && amount.bytes().all(|b| b.is_ascii_digit()) && (amount == "0" || !amount.starts_with('0')),
                    "noncanonical amount"
                );
                out.balances.push(Balance {
                    contract: row.get("contract").filter(|v| !v.is_null()).map(|v| binary(v, 20)).transpose()?,
                    address: binary(&row["address"], 20)?,
                    amount: amount.into(),
                });
            }
        }
        normalize(&mut out)?;
        Ok(out)
    }
    fn stream(path: &Path) -> Result<BTreeMap<u64, Events>> {
        let mut map = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let v: Value = serde_json::from_str(&line?)?;
            ensure!(v["@module"] == "map_events" && v["@type"] == "evm.balances.v1.Events", "stream type");
            let h = v["@block"].as_u64().context("height")?;
            ensure!(h == START + map.len() as u64 && h < STOP, "stream gap/order/duplicate");
            map.insert(h, events(&v["@data"])?);
        }
        ensure!(map.keys().copied().eq(START..STOP), "incomplete stream");
        Ok(map)
    }
    fn clocks(path: &Path) -> Result<BTreeMap<u64, String>> {
        let mut out = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            let (h, tail) = line
                .strip_prefix("----------- BLOCK #")
                .context("clock prefix")?
                .split_once(" (")
                .context("clock height")?;
            let h: u64 = h.replace(',', "").parse()?;
            let (hash, suffix) = tail.split_once(") age=").context("clock hash")?;
            ensure!(suffix.ends_with(" ---------------") && hex::decode(hash)?.len() == 32, "clock format");
            ensure!(h == START + out.len() as u64 && h < STOP, "clock continuity");
            out.insert(h, format!("0x{}", hash.to_ascii_lowercase()));
        }
        ensure!(out.len() == 1024, "clock completeness");
        Ok(out)
    }
    fn reference_binding(hr: &Value, rr: &Value, clock: &BTreeMap<u64, String>) -> Result<()> {
        ensure!(
            rr["chain_id"] == 56 && rr["start"] == START && rr["stop_exclusive"] == STOP && rr["blocks"] == 1024 && rr["reference_sha256"] == CACHE_PINS[3].1,
            "canonical reference identity"
        );
        let bindings = hr["baseline_bindings"].as_array().context("baseline bindings")?;
        let hits: Vec<_> = bindings.iter().filter(|v| v["path"] == CACHE_PINS[3].0).collect();
        ensure!(hits.len() == 1 && hits[0]["sha256"] == CACHE_PINS[3].1, "historical reference binding");
        ensure!(
            hr["blocks"] == 1024
                && hr["configured_profiles"] == 431
                && hr["layouts_sha256"] == bound::BASELINE
                && clock.keys().copied().eq(START..STOP)
                && hr["first_hash"] == clock[&START]
                && hr["last_hash"] == clock[&(STOP - 1)]
                && rr["first_hash"] == hr["first_hash"]
                && rr["last_hash"] == hr["last_hash"],
            "historical interval/fork"
        );
        Ok(())
    }
    fn event_json(events: &Events) -> Value {
        json!({"balances":events.balances.iter().map(|b|json!({"contract":b.contract.as_ref().map(|c|format!("0x{}",hex::encode(c))),"address":format!("0x{}",hex::encode(&b.address)),"amount":b.amount})).collect::<Vec<_>>()})
    }
    fn write_events(file: &mut impl Write, height: u64, events: &Events) -> Result<()> {
        writeln!(
            file,
            "{}",
            json!({"@block":height,"@module":"map_events","@type":"evm.balances.v1.Events","@data":event_json(events)})
        )?;
        Ok(())
    }
    fn key(b: &Balance) -> Key {
        Key {
            contract: b.contract.clone(),
            address: b.address.clone(),
        }
    }
    fn subset(all: &Events, contracts: &BTreeSet<Vec<u8>>, include: bool) -> Events {
        Events {
            balances: all
                .balances
                .iter()
                .filter(|b| b.contract.as_ref().is_some_and(|c| contracts.contains(c)) == include)
                .cloned()
                .collect(),
        }
    }
    fn three_way(baseline: &Events, selected: &Events, combined: &Events, historical: &Events, contracts: &BTreeSet<Vec<u8>>) -> Result<()> {
        ensure!(baseline == historical, "full original431 differs from immutable historical Events");
        ensure!(subset(combined, contracts, false) == *baseline, "combined433 changes baseline payload");
        ensure!(
            subset(combined, contracts, true) == *selected,
            "combined433 selected remainder differs from independent selected2"
        );
        Ok(())
    }
    fn compare_selected(selected: &Events, reference: &Events, ledger: &Ledger) -> Result<(Counts, Vec<Value>)> {
        let mut counts: Counts = Target::ALL
            .into_iter()
            .map(|t| {
                (
                    t.address().to_owned(),
                    [
                        "same_block_reference_matches",
                        "emitted_rows",
                        "emitted_zero_rows",
                        "canonical_rows",
                        "retained_reference_matches",
                        "cold_reference_observations",
                        "cold_nonzero_reference_observations",
                    ]
                    .into_iter()
                    .map(|key| (key.to_owned(), 0))
                    .collect(),
                )
            })
            .collect();
        let refs: BTreeMap<_, _> = reference.balances.iter().map(|b| (key(b), b)).collect();
        let observed: BTreeSet<_> = selected.balances.iter().map(key).collect();
        let mut observations = Vec::new();
        for b in &selected.balances {
            let k = key(b);
            ensure!(
                refs.get(&k).is_some_and(|expected| *expected == b),
                "selected emitted row missing or mismatching full canonical reference"
            );
            let contract = format!("0x{}", hex::encode(b.contract.as_ref().context("selected contract")?));
            let c = counts.get_mut(&contract).context("selected profile")?;
            *c.entry("same_block_reference_matches".into()).or_default() += 1;
            *c.entry("emitted_rows".into()).or_default() += 1;
            *c.entry("emitted_zero_rows".into()).or_default() += u64::from(b.amount == "0");
        }
        for b in &reference.balances {
            let Some(contract) = b.contract.as_ref().map(|c| format!("0x{}", hex::encode(c))) else {
                continue;
            };
            let Some(c) = counts.get_mut(&contract) else { continue };
            *c.entry("canonical_rows".into()).or_default() += 1;
            let k = key(b);
            if observed.contains(&k) {
                continue;
            }
            let known = match ledger.lookup(&k) {
                Lookup::Known(entry) => {
                    ensure!(entry.value == b.amount, "selected retained canonical mismatch");
                    Some(entry.value.clone())
                }
                Lookup::Unknown => None,
                other => anyhow::bail!("unexpected retention {other:?}"),
            };
            *c.entry(
                if known.is_some() {
                    "retained_reference_matches"
                } else {
                    "cold_reference_observations"
                }
                .into(),
            )
            .or_default() += 1;
            if known.is_none() {
                *c.entry("cold_nonzero_reference_observations".into()).or_default() += u64::from(b.amount != "0");
            }
            observations.push(json!({"contract":contract,"address":format!("0x{}",hex::encode(&b.address)),"expected":b.amount,"retained":known,"classification":if known.is_some(){"retained_from_native_emission"}else{"unknown_cold_holder"}}));
        }
        Ok((counts, observations))
    }
    fn add_counts(total: &mut Counts, block: Counts) {
        for (contract, counts) in block {
            let target = total.entry(contract).or_default();
            for (k, n) in counts {
                *target.entry(k).or_default() += n;
            }
        }
    }
    fn run(cache: &Path, out: &Path, report: &mut Value) -> Result<()> {
        let before = bound::snapshot(out)?;
        report["source_inventory_sha256"] = json!(bound::sha(&fs::read(out.join("source-inputs.json"))?));
        let review = bound::review(Path::new("."))?;
        let fixture = Path::new(bound::FIXTURE);
        let raw = fs::read("erc20/balances/tests/fixtures/bsc-refined450-layouts.json")?;
        let candidates = read(&fixture.join("layouts.json"))?;
        let all = bound::combined(&raw, &candidates)?;
        ensure!(
            read(&fixture.join("source-review.json"))? == review,
            "candidate review differs from freshly verified Phase A"
        );
        bound::save(&out.join("candidate-combined433.json"), &all)?;
        bound::save(&out.join("source-review.json"), &review)?;
        let mut inputs = Vec::new();
        for (path, pin) in CACHE_PINS {
            let bytes = fs::read(cache.join(path))?;
            ensure!(bound::sha(&bytes) == pin, "immutable cache input changed {path}");
            inputs.push(json!({"path":cache.join(path),"sha256":pin,"bytes":bytes.len()}));
        }
        for p in [
            "erc20/balances/tests/fixtures/bsc-refined450-layouts.json".to_owned(),
            format!("{}/layouts.json", bound::FIXTURE),
            format!("{}/source-review.json", bound::FIXTURE),
        ] {
            let bytes = fs::read(&p)?;
            inputs.push(json!({"path":p,"sha256":bound::sha(&bytes),"bytes":bytes.len()}));
        }
        report["inputs"] = json!(inputs);
        report["phase_a_artifacts"] = review["phase_a_artifacts"].clone();
        let historical = stream(&cache.join(CACHE_PINS[0].0))?;
        let reference = stream(&cache.join(CACHE_PINS[3].0))?;
        let hr = read(&cache.join(CACHE_PINS[1].0))?;
        let rr = read(&cache.join(CACHE_PINS[4].0))?;
        let survey = read(&cache.join(CACHE_PINS[5].0))?;
        let mut historical_profiles = Vec::new();
        for t in Target::ALL {
            let hits: Vec<_> = survey["tokens"]
                .as_array()
                .context("survey tokens")?
                .iter()
                .filter(|p| p["contract"] == t.address())
                .collect();
            ensure!(
                hits.len() == 1 && hits[0]["runtime_hashes"] == json!([t.runtime_hash(), t.runtime_hash()]) && hits[0]["runtime_stable_at_boundaries"] == true,
                "historical boundary runtime binding"
            );
            historical_profiles.push(hits[0].clone());
        }
        report["historical_profile_evidence"] = json!(historical_profiles);
        let clock = clocks(&cache.join(CACHE_PINS[2].0))?;
        let original_blocks = block_manifest(&read(&cache.join(CACHE_PINS[6].0))?, &clock)?;
        reference_binding(&hr, &rr, &clock)?;
        let parse = |s: &str| erc20_balances::layout::parse(s).map_err(|e| anyhow::anyhow!(e.to_string()));
        let baseline = parse(std::str::from_utf8(&raw)?)?;
        let selected = parse(&candidates.to_string())?;
        let combined = parse(&all.to_string())?;
        let contracts: BTreeSet<_> = selected.iter().map(|l| l.contract.clone()).collect();
        let mut ledger = Ledger::new(1, Domain::Balances);
        let mut counts = BTreeMap::new();
        let mut versions = BTreeMap::<i32, u64>::new();
        let mut total_baseline = 0;
        let mut total_selected = 0;
        let mut prev = hr["first_parent_hash"].as_str().context("first parent")?.to_owned();
        let mut baseline_file = BufWriter::new(File::create(out.join("baseline-events.jsonl"))?);
        let mut selected_file = BufWriter::new(File::create(out.join("selected-events.jsonl"))?);
        let mut combined_file = BufWriter::new(File::create(out.join("combined-events.jsonl"))?);
        let mut blocks = BufWriter::new(File::create(out.join("blocks.jsonl"))?);
        let mut perblock = BufWriter::new(File::create(out.join("per-block.jsonl"))?);
        let mut cold = BufWriter::new(File::create(out.join("reference-only-selected.jsonl"))?);
        let mut metadata = BufWriter::new(File::create(out.join("metadata-writes.jsonl"))?);
        let mut metadata_counts: Counts = Target::ALL
            .into_iter()
            .map(|t| (t.address().to_owned(), [("changing_records".into(), 0), ("equal_records".into(), 0)].into()))
            .collect();
        let mut original_refusals = Vec::new();
        for h in START..STOP {
            report["attempted_block"] = json!(h);
            let path = cache.join(format!("out/top50-full-holder-blocks/{h}.pb"));
            let bytes = fs::read(&path)?;
            verify_block_bytes(&bytes, h, &original_blocks)?;
            let block = eth::Block::decode(bytes.as_slice())?;
            let header = block.header.as_ref().context("header")?;
            let hash = format!("0x{}", hex::encode(&block.hash));
            let parent = format!("0x{}", hex::encode(&header.parent_hash));
            ensure!(
                block.number == h
                    && header.number == h
                    && hash == clock[&h]
                    && parent == prev
                    && matches!(block.ver, 3..=5)
                    && block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32,
                "saved block identity/version/detail at {h}"
            );
            let mut original = erc20_balances::project(&block, &baseline).map_err(|e| anyhow::anyhow!("baseline at {h}: {e}"))?;
            if [122288595, 122288639, 122288684, 122288729, 122288791].contains(&h) {
                let mut hypothesis = candidates[1].clone();
                hypothesis.as_object_mut().unwrap().remove("metadata_semantics");
                let error = erc20_balances::project(&block, &parse(&json!([hypothesis]).to_string())?)
                    .err()
                    .context("historical COOKIE refusal must remain visible")?
                    .to_string();
                let slot = if h == 122288791 { 11 } else { 3 };
                ensure!(error.contains(&format!("at key 0x{slot:064x}")), "historical COOKIE refusal changed");
                original_refusals.push(json!({"block":h,"error":error,"slot":slot,"block_sha256":bound::sha(&bytes)}));
            }
            let mut own = erc20_balances::project(&block, &selected).map_err(|e| anyhow::anyhow!("selected at {h}: {e}"))?;
            let mut all = erc20_balances::project(&block, &combined).map_err(|e| anyhow::anyhow!("combined at {h}: {e}"))?;
            normalize(&mut original)?;
            normalize(&mut own)?;
            normalize(&mut all)?;
            three_way(&original, &own, &all, &historical[&h], &contracts)?;
            let mut staged = ledger.clone();
            staged.apply(
                &Clock {
                    number: h,
                    hash: block.hash.clone(),
                    parent_hash: header.parent_hash.clone(),
                },
                &own.balances,
            )?;
            let (block_counts, observations) = compare_selected(&own, &reference[&h], &staged)?;
            let metadata_rows = bound::metadata_observations(&block, &review)?;
            // Publish a completed block only after all three projections and oracle checks.
            write_events(&mut baseline_file, h, &original)?;
            write_events(&mut selected_file, h, &own)?;
            write_events(&mut combined_file, h, &all)?;
            for observation in observations {
                writeln!(cold, "{}", json!({"block":h,"observation":observation}))?;
            }
            for observation in metadata_rows {
                let contract = observation["contract"].as_str().context("metadata contract")?.to_owned();
                let counter = metadata_counts.entry(contract).or_default();
                *counter
                    .entry(
                        if observation["equal_record"] == true {
                            "equal_records"
                        } else {
                            "changing_records"
                        }
                        .into(),
                    )
                    .or_default() += 1;
                if observation["equal_record"] == false {
                    for field in observation["source_fields"].as_array().context("metadata fields")? {
                        *counter.entry(format!("changed_field:{}", field.as_str().context("field")?)).or_default() += 1;
                    }
                }
                writeln!(metadata, "{}", json!({"block":h,"observation":observation}))?;
            }
            writeln!(
                blocks,
                "{}",
                json!({"block":h,"hash":hash,"parent_hash":parent,"producer_version":block.ver,"path":path,"bytes":bytes.len(),"sha256":bound::sha(&bytes),"historical_protobuf_sha256":bound::sha(&historical[&h].encode_to_vec()),"baseline_protobuf_sha256":bound::sha(&original.encode_to_vec()),"selected_protobuf_sha256":bound::sha(&own.encode_to_vec()),"combined_protobuf_sha256":bound::sha(&all.encode_to_vec())})
            )?;
            writeln!(
                perblock,
                "{}",
                json!({"block":h,"baseline_rows":original.balances.len(),"selected_rows":own.balances.len(),"combined_rows":all.balances.len(),"selected":block_counts})
            )?;
            ledger = staged;
            add_counts(&mut counts, block_counts);
            total_baseline += original.balances.len();
            total_selected += own.balances.len();
            *versions.entry(block.ver).or_default() += 1;
            prev = hash;
            report["last_completed_block"] = json!(h);
            report["completed_blocks"] = json!(h - START + 1);
            report["baseline_rows"] = json!(total_baseline);
            report["selected_rows"] = json!(total_selected);
            report["combined_rows"] = json!(total_baseline + total_selected);
            report["selected"] = json!(counts);
            report["producer_versions"] = json!(versions);
            report["retention"] = json!(ledger.report());
            report["metadata_observations"] = json!(metadata_counts);
            report["reproduced_original_cookie_refusals"] = json!(original_refusals);
            if (h - START + 1) % 128 == 0 {
                eprintln!("Offline candidate replay: {} / 1024", h - START + 1);
            }
        }
        ensure!(total_baseline == 110139, "original baseline row total");
        ensure!(original_refusals.len() == 5, "all five original COOKIE refusals reproduced");
        for file in [
            &mut baseline_file,
            &mut selected_file,
            &mut combined_file,
            &mut blocks,
            &mut perblock,
            &mut cold,
            &mut metadata,
        ] {
            file.flush()?;
        }
        for t in Target::ALL {
            let address = hex::decode(&t.address()[2..])?;
            let c = counts.entry(t.address().into()).or_default();
            let entries: Vec<_> = ledger.entries().iter().filter(|(k, _)| k.contract.as_ref() == Some(&address)).collect();
            c.insert("initialized_observed_holders".into(), entries.len() as u64);
            c.insert("known_zero_holders".into(), entries.iter().filter(|(_, v)| v.value == "0").count() as u64);
            let holders: BTreeSet<_> = reference
                .values()
                .flat_map(|e| &e.balances)
                .filter(|b| b.contract.as_ref() == Some(&address))
                .map(|b| b.address.clone())
                .collect();
            c.insert("canonical_distinct_holders".into(), holders.len() as u64);
        }
        ensure!(bound::inventory()? == before, "candidate replay source drift");
        ensure!(bound::review(Path::new("."))? == review, "frozen proof drift");
        for (path, pin) in CACHE_PINS {
            ensure!(bound::sha(&fs::read(cache.join(path))?) == pin, "cache drift");
        }
        for input in report["inputs"].as_array().context("input bindings")? {
            let path = input["path"].as_str().context("input path")?;
            ensure!(bound::sha(&fs::read(path)?) == input["sha256"], "replay input drift {path}");
        }
        report["selected"] = json!(counts);
        report["first_hash"] = hr["first_hash"].clone();
        report["last_hash"] = hr["last_hash"].clone();
        report["baseline_all_fields_equal"] = json!(true);
        report["combined_projection_and_independent_selected_equal"] = json!(true);
        report["artifacts"] = json!([
            "baseline-events.jsonl",
            "selected-events.jsonl",
            "combined-events.jsonl",
            "blocks.jsonl",
            "per-block.jsonl",
            "reference-only-selected.jsonl",
            "metadata-writes.jsonl"
        ]
        .into_iter()
        .map(|p| -> Result<Value> { Ok(json!({"path":p,"sha256":bound::sha(&fs::read(out.join(p))?)})) })
        .collect::<Result<Vec<_>>>()?);
        report["status"] = json!("passed");
        Ok(())
    }
    pub fn main() -> Result<()> {
        let a: Vec<_> = std::env::args().collect();
        ensure!(
            a.len() == 3,
            "usage: replay_bnbtiger_cookie_candidates <original erc20/balances cache> <fresh output>"
        );
        let out = Path::new(&a[2]);
        ensure!(!out.exists(), "fresh output required");
        fs::create_dir_all(out)?;
        let mut report = json!({"status":"incomplete","qualified":false,"mode":"offline_saved_data_only","chain_id":56,"start_inclusive":START,"stop_exclusive":STOP,"completed_blocks":0,"network_requests":0,"historical_profiles":431,"selected_profiles":2,"combined_profiles":433,"limits":bound::LIMITS});
        let result = run(Path::new(&a[1]), out, &mut report);
        if let Err(e) = &result {
            report["status"] = json!("failed");
            report["error"] = json!(format!("{e:#}"));
        }
        bound::save(&out.join("report.json"), &report)?;
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn row(t: usize, holder: u8, amount: &str) -> Balance {
            Balance {
                contract: Some(hex::decode(&Target::ALL[t].address()[2..]).unwrap()),
                address: vec![holder; 20],
                amount: amount.into(),
            }
        }
        #[test]
        fn full_payload_three_way_refuses_any_baseline_or_selected_change() {
            let b = Events {
                balances: vec![Balance {
                    contract: Some(vec![7; 20]),
                    address: vec![8; 20],
                    amount: "9".into(),
                }],
            };
            let s = Events {
                balances: vec![row(0, 1, "123")],
            };
            let mut all = b.clone();
            all.balances.extend(s.balances.clone());
            let targets = Target::ALL.map(|t| hex::decode(&t.address()[2..]).unwrap()).into_iter().collect();
            three_way(&b, &s, &all, &b, &targets).unwrap();
            let mut bad = all.clone();
            bad.balances[0].contract = None;
            assert!(three_way(&b, &s, &bad, &b, &targets).is_err());
            let mut bad = all;
            bad.balances[1].amount = "124".into();
            assert!(three_way(&b, &s, &bad, &b, &targets).is_err());
        }
        #[test]
        fn missing_or_changed_selected_oracle_never_passes() {
            let native = Events {
                balances: vec![row(0, 1, "0")],
            };
            let ledger = Ledger::new(1, Domain::Balances);
            assert!(compare_selected(&native, &Events::default(), &ledger).is_err());
            assert!(compare_selected(
                &native,
                &Events {
                    balances: vec![row(0, 1, "1")]
                },
                &ledger
            )
            .is_err());
            compare_selected(&native, &native, &ledger).unwrap();
        }
        #[test]
        fn cold_zero_is_unknown_and_only_native_history_establishes_known_zero() {
            let mut ledger = Ledger::new(1, Domain::Balances);
            let reference = Events {
                balances: vec![row(0, 1, "0"), row(0, 2, "123")],
            };
            let (c, _) = compare_selected(&Events::default(), &reference, &ledger).unwrap();
            assert_eq!(c[Target::Bnbtiger.address()]["cold_reference_observations"], 2);
            assert_eq!(c[Target::Bnbtiger.address()]["cold_nonzero_reference_observations"], 1);
            ledger
                .apply(
                    &Clock {
                        number: 1,
                        hash: vec![1; 32],
                        parent_hash: vec![0; 32],
                    },
                    &reference.balances[..1],
                )
                .unwrap();
            let (c, _) = compare_selected(&Events::default(), &reference, &ledger).unwrap();
            assert_eq!(c[Target::Bnbtiger.address()]["retained_reference_matches"], 1);
            assert_eq!(c[Target::Bnbtiger.address()]["cold_reference_observations"], 1);
            let wrong = Events {
                balances: vec![row(0, 1, "1")],
            };
            assert!(compare_selected(&Events::default(), &wrong, &ledger).is_err());
        }
        #[test]
        fn duplicate_extra_fields_and_noncanonical_values_refuse() {
            let v = json!({"balances":[{"contract":format!("0x{}","11".repeat(20)),"address":format!("0x{}","22".repeat(20)),"amount":"1"}]});
            events(&v).unwrap();
            for field in ["unexpected", "timestamp"] {
                let mut bad = v.clone();
                bad[field] = json!(1);
                assert!(events(&bad).is_err());
            }
            let mut bad = v.clone();
            bad["balances"].as_array_mut().unwrap().push(v["balances"][0].clone());
            assert!(events(&bad).is_err());
            for amount in ["", "01", "-1", "1.0"] {
                let mut bad = v.clone();
                bad["balances"][0]["amount"] = json!(amount);
                assert!(events(&bad).is_err());
            }
        }

        #[test]
        fn reference_identity_rejects_network_fork_interval_and_rebound_oracle() {
            let clocks = (START..STOP).map(|h| (h, format!("hash{h}"))).collect::<BTreeMap<_, _>>();
            let hr = json!({"blocks":1024,"configured_profiles":431,"layouts_sha256":bound::BASELINE,"first_hash":clocks[&START],"last_hash":clocks[&(STOP-1)],"baseline_bindings":[{"path":CACHE_PINS[3].0,"sha256":CACHE_PINS[3].1}]});
            let rr = json!({"chain_id":56,"start":START,"stop_exclusive":STOP,"blocks":1024,"reference_sha256":CACHE_PINS[3].1,"first_hash":clocks[&START],"last_hash":clocks[&(STOP-1)]});
            reference_binding(&hr, &rr, &clocks).unwrap();
            for (key, value) in [
                ("chain_id", json!(1)),
                ("start", json!(START + 1)),
                ("stop_exclusive", json!(STOP - 1)),
                ("blocks", json!(1023)),
                ("first_hash", json!("other")),
                ("last_hash", json!("other")),
                ("reference_sha256", json!("replacement")),
            ] {
                let mut bad = rr.clone();
                bad[key] = value;
                assert!(reference_binding(&hr, &bad, &clocks).is_err(), "{key}");
            }
            let mut missing = clocks.clone();
            missing.remove(&(START + 1));
            assert!(reference_binding(&hr, &rr, &missing).is_err());
            let mut duplicate = hr.clone();
            duplicate["baseline_bindings"].as_array_mut().unwrap().push(hr["baseline_bindings"][0].clone());
            assert!(reference_binding(&duplicate, &rr, &clocks).is_err());
        }

        #[test]
        fn failed_later_comparison_does_not_mutate_ledger_or_completed_counts() {
            let mut ledger = Ledger::new(1, Domain::Balances);
            ledger
                .apply(
                    &Clock {
                        number: 1,
                        hash: vec![1; 32],
                        parent_hash: vec![0; 32],
                    },
                    &[row(0, 1, "123")],
                )
                .unwrap();
            let original = ledger.report();
            let mut staged = ledger.clone();
            let native = Events {
                balances: vec![row(0, 1, "124")],
            };
            staged
                .apply(
                    &Clock {
                        number: 2,
                        hash: vec![2; 32],
                        parent_hash: vec![1; 32],
                    },
                    &native.balances,
                )
                .unwrap();
            assert!(compare_selected(&native, &Events::default(), &staged).is_err());
            assert_eq!(ledger.report(), original);
            assert!(matches!(ledger.lookup(&key(&native.balances[0])),Lookup::Known(entry) if entry.value=="123"));
        }

        #[test]
        fn original_pb_manifest_refuses_changed_trace_bytes_with_identical_header() {
            let clocks = (START..STOP).map(|h| (h, format!("0x{h:064x}"))).collect::<BTreeMap<_, _>>();
            let mut block = eth::Block {
                number: START,
                ver: 5,
                hash: vec![1; 32],
                header: Some(eth::BlockHeader {
                    number: START,
                    ..Default::default()
                }),
                ..Default::default()
            };
            let original = block.encode_to_vec();
            let rows = json!((START..STOP)
                .map(|h| json!({"block":h,"hash":clocks[&h],"sha256":bound::sha(&original)}))
                .collect::<Vec<_>>());
            let pins = block_manifest(&rows, &clocks).unwrap();
            verify_block_bytes(&original, START, &pins).unwrap();
            block.transaction_traces.push(eth::TransactionTrace::default());
            let changed = block.encode_to_vec();
            assert_eq!(eth::Block::decode(original.as_slice()).unwrap().header, block.header);
            assert!(verify_block_bytes(&changed, START, &pins).is_err());
            let mut missing = rows.clone();
            missing.as_array_mut().unwrap().pop();
            assert!(block_manifest(&missing, &clocks).is_err());
            let mut reordered = rows.clone();
            reordered.as_array_mut().unwrap().swap(0, 1);
            assert!(block_manifest(&reordered, &clocks).is_err());
            let mut changed = rows;
            changed[0]["hash"] = json!("other");
            assert!(block_manifest(&changed, &clocks).is_err());
        }
    }
}
