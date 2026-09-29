#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    offline::run()
}

#[cfg(not(target_arch = "wasm32"))]
mod offline {
    use anyhow::{ensure, Context, Result};
    use erc20_balances_tools::{tops_lpinfo as bound, tops_proof::sha};
    use evm_retention::{Clock, Domain, Key, Ledger, Lookup};
    use prost::Message;
    use proto::pb::evm::balances::v1::{Balance, Events};
    use serde_json::{json, Value};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        io::{BufRead, BufReader, Write},
        path::Path,
    };
    use substreams_ethereum::pb::eth::v2 as eth;
    const START: u64 = 123561000;
    const STOP: u64 = 123562024;
    const REFERENCES: &str = "out/live-2026-09-23/ranking/reference.jsonl";
    const REFERENCE_SHA: &str = "0b9c49491f7ed4e698045aabc368bc668ff64cb3ff47618cef56897ecc5c7d30";
    const CLOCKS: &str = "out/live-2026-09-23/ranking/reference.clocks.txt";
    const CLOCK_SHA: &str = "241cd9fd6ba1b7ff8e0ca0dd823b7775d10879951e8ae6624583896f51d5cf54";
    const BLOCKS: &str = "docs/evidence/bsc-exclusions-20260928/blocks.json";
    const BLOCKS_SHA: &str = "80dd087365e8e0457b7d397bef45ece8571b254c270282403d4a7e6e0425fe9f";
    fn binary(v: &Value, size: usize) -> Result<Vec<u8>> {
        let s = v.as_str().context("hex")?;
        ensure!(s.starts_with("0x"), "canonical hex");
        let b = hex::decode(&s[2..])?;
        ensure!(b.len() == size, "hex width");
        Ok(b)
    }
    fn balance_key(b: &Balance) -> Key {
        Key {
            contract: b.contract.clone(),
            address: b.address.clone(),
        }
    }
    fn event_json(e: &Events) -> Value {
        json!({"balances":e.balances.iter().map(|b|json!({"contract":b.contract.as_ref().map(|c|format!("0x{}",hex::encode(c))),"address":format!("0x{}",hex::encode(&b.address)),"amount":b.amount})).collect::<Vec<_>>()})
    }
    fn references(raw: &[u8]) -> Result<BTreeMap<u64, Events>> {
        ensure!(sha(raw) == REFERENCE_SHA, "immutable whole canonical reference");
        let mut out = BTreeMap::new();
        for line in BufReader::new(raw).lines() {
            let v: Value = serde_json::from_str(&line?)?;
            ensure!(
                v["@module"] == "map_events" && v["@type"] == "evm.balances.v1.Events",
                "reference module/schema"
            );
            let h = v["@block"].as_u64().context("height")?;
            ensure!(h == START + out.len() as u64 && h < STOP, "reference contiguous order");
            ensure!(
                v["@data"].as_object().context("Events")?.keys().all(|k| k == "balances"),
                "unknown Events field"
            );
            let mut e = Events::default();
            let mut keys = BTreeSet::new();
            for row in v["@data"]["balances"].as_array().context("balances")? {
                ensure!(
                    row.as_object()
                        .context("row")?
                        .keys()
                        .all(|k| ["contract", "address", "amount"].contains(&k.as_str())),
                    "unknown Balance field"
                );
                let contract = binary(&row["contract"], 20)?;
                let address = binary(&row["address"], 20)?;
                let amount = row["amount"].as_str().context("amount")?;
                ensure!(
                    !amount.is_empty() && amount.bytes().all(|b| b.is_ascii_digit()) && (amount == "0" || !amount.starts_with('0')),
                    "canonical amount"
                );
                ensure!(keys.insert((contract.clone(), address.clone())), "duplicate reference balance");
                if row["contract"] == bound::CONTRACT {
                    e.balances.push(Balance {
                        contract: Some(contract),
                        address,
                        amount: amount.into(),
                    });
                }
            }
            e.balances.sort_by_key(balance_key);
            out.insert(h, e);
        }
        ensure!(out.len() == 1024, "complete canonical interval");
        Ok(out)
    }
    fn clocks(raw: &[u8]) -> Result<BTreeMap<u64, Vec<u8>>> {
        ensure!(sha(raw) == CLOCK_SHA, "immutable canonical clocks");
        let mut out = BTreeMap::new();
        for line in BufReader::new(raw).lines() {
            let line = line?;
            let (height, tail) = line
                .strip_prefix("----------- BLOCK #")
                .context("clock prefix")?
                .split_once(" (")
                .context("clock syntax")?;
            let h = height.replace(',', "").parse::<u64>()?;
            let hash = hex::decode(tail.split(')').next().context("hash")?)?;
            ensure!(h == START + out.len() as u64 && h < STOP && hash.len() == 32, "clock sequence/width");
            out.insert(h, hash);
        }
        ensure!(out.len() == 1024, "clock completeness");
        Ok(out)
    }
    fn manifests(raw: &[u8], clocks: &BTreeMap<u64, Vec<u8>>) -> Result<BTreeMap<u64, String>> {
        ensure!(sha(raw) == BLOCKS_SHA, "immutable original whole-PB inventory");
        let rows: Vec<Value> = serde_json::from_slice(raw)?;
        ensure!(rows.len() == 1024, "PB manifest length");
        let mut out = BTreeMap::new();
        for (h, v) in (START..STOP).zip(rows) {
            ensure!(v["block"] == h && binary(&v["hash"], 32)? == clocks[&h], "PB manifest/canonical clock identity");
            let digest = v["sha256"].as_str().context("PB hash")?;
            ensure!(digest.len() == 64 && hex::decode(digest)?.len() == 32, "PB hash width");
            out.insert(h, digest.into());
        }
        Ok(out)
    }
    fn verify_block(raw: &[u8], height: u64, pins: &BTreeMap<u64, String>, clocks: &BTreeMap<u64, Vec<u8>>, previous: Option<&[u8]>) -> Result<eth::Block> {
        ensure!(pins.get(&height) == Some(&sha(raw)), "whole original PB changed at{height}");
        let b = eth::Block::decode(raw)?;
        let header = b.header.as_ref().context("header")?;
        ensure!(
            b.number == height && header.number == height && b.hash == clocks[&height] && header.parent_hash.len() == 32 && matches!(b.ver, 4 | 5),
            "block clock/producer identity"
        );
        if let Some(prev) = previous {
            ensure!(header.parent_hash == prev, "parent discontinuity");
        }
        Ok(b)
    }
    #[derive(Default, serde::Serialize)]
    struct Coverage {
        emitted_rows: u64,
        emitted_zero_rows: u64,
        same_block_reference_matches: u64,
        retained_only_reference_matches: u64,
        cold_reference_observations: u64,
        cold_nonzero_reference_observations: u64,
        canonical_observations: u64,
    }
    fn compare(e: &Events, reference: &Events, ledger: &Ledger) -> Result<Coverage> {
        let refs: BTreeMap<_, _> = reference.balances.iter().map(|r| (balance_key(r), r)).collect();
        let emitted: BTreeSet<_> = e.balances.iter().map(balance_key).collect();
        ensure!(emitted.len() == e.balances.len(), "duplicate emitted key");
        let mut count = Coverage::default();
        for row in &e.balances {
            ensure!(
                refs.get(&balance_key(row)).copied() == Some(row),
                "complete emitted row lacks exact same-block canonical match"
            );
            count.emitted_rows += 1;
            count.emitted_zero_rows += u64::from(row.amount == "0");
            count.same_block_reference_matches += 1;
        }
        for row in &reference.balances {
            count.canonical_observations += 1;
            let k = balance_key(row);
            match ledger.lookup(&k) {
                Lookup::Known(v) => {
                    ensure!(v.value == row.amount, "retained amount mismatch");
                    if !emitted.contains(&k) {
                        count.retained_only_reference_matches += 1;
                    }
                }
                Lookup::Unknown => {
                    count.cold_reference_observations += 1;
                    count.cold_nonzero_reference_observations += u64::from(row.amount != "0");
                }
                other => anyhow::bail!("unexpected ledger result {other:?}"),
            }
        }
        Ok(count)
    }
    fn sum(a: &mut Coverage, b: &Coverage) {
        a.emitted_rows += b.emitted_rows;
        a.emitted_zero_rows += b.emitted_zero_rows;
        a.same_block_reference_matches += b.same_block_reference_matches;
        a.retained_only_reference_matches += b.retained_only_reference_matches;
        a.cold_reference_observations += b.cold_reference_observations;
        a.cold_nonzero_reference_observations += b.cold_nonzero_reference_observations;
        a.canonical_observations += b.canonical_observations;
    }
    pub fn run() -> Result<()> {
        let args: Vec<_> = std::env::args().collect();
        ensure!(args.len() == 3, "usage: replay_tops_lpinfo ORIGINAL_PACKAGE FRESH_OUTPUT");
        let cache = Path::new(&args[1]);
        let out = Path::new(&args[2]);
        fs::create_dir(out)?;
        let package = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut blocks = vec![];
        let mut attempts = vec![];
        let mut observations = vec![];
        let mut coverage = Coverage::default();
        let mut counts = bound::Counts::default();
        let mut holders = BTreeSet::new();
        let mut ledger = Ledger::new(1, Domain::Balances);
        let mut attempted = 0;
        let mut source_digest = String::new();
        let mut inputs = BTreeMap::new();
        let mut events_file = fs::File::create(out.join("events.jsonl"))?;
        let result = (|| -> Result<()> {
            source_digest = bound::snapshot(out)?;
            bound::verify_cache(package, cache)?;
            let baseline = fs::read(package.join("tests/fixtures/bsc-refined450-layouts.json"))?;
            let candidate_raw = fs::read(package.join(bound::FIXTURE).join("layouts.json"))?;
            let candidate: Value = serde_json::from_slice(&candidate_raw)?;
            bound::verify_candidate(&baseline, &candidate)?;
            let review_raw = fs::read(package.join(bound::FIXTURE).join("source-review.json"))?;
            ensure!(serde_json::from_slice::<Value>(&review_raw)? == bound::review(package)?, "frozen source review");
            let layouts = erc20_balances::layout::parse(&candidate.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let refs_raw = fs::read(cache.join(REFERENCES))?;
            let clock_raw = fs::read(cache.join(CLOCKS))?;
            let block_raw = fs::read(package.join(BLOCKS))?;
            for (name, raw) in [
                (REFERENCES, &refs_raw),
                (CLOCKS, &clock_raw),
                (BLOCKS, &block_raw),
                ("candidate", &candidate_raw),
                ("source_review", &review_raw),
                ("baseline431", &baseline),
            ] {
                inputs.insert(name.to_owned(), json!({"sha256":sha(raw),"bytes":raw.len()}));
            }
            let refs = references(&refs_raw)?;
            let clock = clocks(&clock_raw)?;
            let pins = manifests(&block_raw, &clock)?;
            let mut previous: Option<Vec<u8>> = None;
            for height in START..STOP {
                attempted += 1;
                let raw = fs::read(cache.join(format!("out/live-2026-09-23/firehose-blocks/{height}.pb")))?;
                attempts.push(json!({"block":height,"sha256":sha(&raw),"bytes":raw.len(),"completed":false}));
                let b = verify_block(&raw, height, &pins, &clock, previous.as_deref())?;
                let (mut events, observed) = bound::project_counted(&b, &layouts).with_context(|| format!("mapper refused block{height}"))?;
                events.balances.sort_by_key(balance_key);
                let header = b.header.as_ref().unwrap();
                let mut staged_ledger = ledger.clone();
                staged_ledger.apply(
                    &Clock {
                        number: height,
                        hash: b.hash.clone(),
                        parent_hash: header.parent_hash.clone(),
                    },
                    &events.balances,
                )?;
                let checked = compare(&events, &refs[&height], &staged_ledger)?;
                ledger = staged_ledger;
                sum(&mut coverage, &checked);
                for row in &events.balances {
                    holders.insert(balance_key(row));
                }
                counts.validated_appends += observed.validated_appends;
                counts.validated_cleanups += observed.validated_cleanups;
                counts.observed_length_decrements += observed.observed_length_decrements;
                counts.observed_metadata_stores += observed.observed_metadata_stores;
                counts.observed_equal_metadata_stores += observed.observed_equal_metadata_stores;
                let hash = format!("0x{}", hex::encode(&b.hash));
                writeln!(
                    events_file,
                    "{}",
                    json!({"@block":height,"@module":"map_events","@type":"evm.balances.v1.Events","@data":event_json(&events)})
                )?;
                observations.push(json!({"block":height,"hash":hash,"coverage":checked,"operations":observed}));
                blocks.push(json!({"block":height,"hash":hash,"parent_hash":format!("0x{}",hex::encode(&header.parent_hash)),"version":b.ver,"sha256":sha(&raw),"bytes":raw.len(),"events_sha256":sha(&events.encode_to_vec())}));
                attempts.last_mut().unwrap()["completed"] = json!(true);
                previous = Some(b.hash);
            }
            for (path, raw) in [
                (cache.join(REFERENCES), refs_raw),
                (cache.join(CLOCKS), clock_raw),
                (package.join(BLOCKS), block_raw),
                (package.join(bound::FIXTURE).join("layouts.json"), candidate_raw),
                (package.join(bound::FIXTURE).join("source-review.json"), review_raw),
                (package.join("tests/fixtures/bsc-refined450-layouts.json"), baseline),
            ] {
                ensure!(fs::read(path)? == raw, "input changed during replay");
            }
            bound::review(package)?;
            let after = out.join("after");
            fs::create_dir(&after)?;
            ensure!(bound::snapshot(&after)? == source_digest, "source changed during replay");
            Ok(())
        })();
        events_file.flush()?;
        fs::write(out.join("blocks.json"), serde_json::to_vec_pretty(&blocks)?)?;
        fs::write(out.join("observations.json"), serde_json::to_vec_pretty(&observations)?)?;
        fs::write(out.join("attempts.json"), serde_json::to_vec_pretty(&attempts)?)?;
        let mut artifacts = BTreeMap::new();
        for file in ["events.jsonl", "blocks.json", "observations.json", "attempts.json"] {
            let raw = fs::read(out.join(file))?;
            artifacts.insert(file, json!({"sha256":sha(&raw),"bytes":raw.len()}));
        }
        let report = json!({"status":if result.is_ok(){"passed"}else{"failed"},"qualified":false,"network_requests":0,"scope":bound::LIMITS,"interval":{"start":START,"stop_exclusive":STOP,"requested_blocks":1024,"attempted_blocks":attempted,"completed_blocks":blocks.len(),"complete":result.is_ok()},"first_failed_block":(attempted>blocks.len() as u64).then_some(START+blocks.len() as u64),"failure":result.as_ref().err().map(|e|format!("{e:#}")),"source_inventory_sha256":source_digest,"inputs":inputs,"artifacts":artifacts,"coverage":coverage,"operations":counts,"initialized_observed_holders":holders.len(),"ledger":ledger.report(),"initialization":"emitted rows only; no checkpoint or canonical reference seeding","historical431_sha256":bound::BASELINE,"baseline_cohort":"unchanged historical431 and qualified425; replay projects TOPS separately"});
        fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
        result.context("TOPS saved replay stopped; partial report and outputs preserved")
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn sample(amount: &str) -> Balance {
            Balance {
                contract: Some(hex::decode(&bound::CONTRACT[2..]).unwrap()),
                address: vec![7; 20],
                amount: amount.into(),
            }
        }
        fn clock(number: u64) -> Clock {
            Clock {
                number,
                hash: vec![number as u8; 32],
                parent_hash: vec![(number - 1) as u8; 32],
            }
        }
        #[test]
        fn original_manifest_is_complete_and_bound_to_canonical_clocks() {
            let raw = include_bytes!("../../../docs/evidence/bsc-exclusions-20260928/blocks.json");
            let rows: Vec<Value> = serde_json::from_slice(raw).unwrap();
            let mut c: BTreeMap<_, _> = rows.iter().map(|v| (v["block"].as_u64().unwrap(), binary(&v["hash"], 32).unwrap())).collect();
            assert_eq!(manifests(raw, &c).unwrap().len(), 1024);
            c.get_mut(&START).unwrap()[0] ^= 1;
            assert!(manifests(raw, &c).is_err());
            let mut changed = raw.to_vec();
            changed.push(b' ');
            assert!(manifests(&changed, &c).is_err());
        }
        #[test]
        fn matching_header_does_not_allow_changed_pb_storage_or_version() {
            let raw = include_bytes!("../../../tests/fixtures/bsc-exclusions-20260928/tops-123561227-tx63.pb");
            let block = eth::Block::decode(raw.as_slice()).unwrap();
            let h = block.number;
            let pins = BTreeMap::from([(h, sha(raw))]);
            let clocks = BTreeMap::from([(h, block.hash.clone())]);
            verify_block(raw, h, &pins, &clocks, None).unwrap();
            let mut altered = block.clone();
            altered.transaction_traces[0].calls[0].input.push(0);
            assert!(verify_block(&altered.encode_to_vec(), h, &pins, &clocks, None).is_err());
            let mut altered = block;
            altered.ver = 3;
            let raw = altered.encode_to_vec();
            assert!(verify_block(&raw, h, &BTreeMap::from([(h, sha(&raw))]), &clocks, None).is_err());
        }
        #[test]
        fn complete_same_block_rows_and_unique_emissions_are_required() {
            let e = Events { balances: vec![sample("7")] };
            let mut ledger = Ledger::new(1, Domain::Balances);
            ledger.apply(&clock(1), &e.balances).unwrap();
            let c = compare(&e, &e, &ledger).unwrap();
            assert_eq!(c.same_block_reference_matches, 1);
            assert!(compare(&e, &Events::default(), &ledger).is_err());
            let mut wrong = e.clone();
            wrong.balances[0].address[0] ^= 1;
            assert!(compare(&e, &wrong, &ledger).is_err());
            wrong = e.clone();
            wrong.balances[0].amount = "8".into();
            assert!(compare(&e, &wrong, &ledger).is_err());
            let duplicated = Events {
                balances: vec![sample("7"), sample("7")],
            };
            assert!(compare(&duplicated, &e, &ledger).is_err());
        }
        #[test]
        fn cold_zero_reference_never_initializes_and_retained_only_is_separate() {
            let mut ledger = Ledger::new(1, Domain::Balances);
            let empty = Events::default();
            for amount in ["0", "9"] {
                let reference = Events {
                    balances: vec![sample(amount)],
                };
                let c = compare(&empty, &reference, &ledger).unwrap();
                assert_eq!(c.cold_reference_observations, 1);
                assert_eq!(c.cold_nonzero_reference_observations, u64::from(amount != "0"));
                assert!(matches!(ledger.lookup(&balance_key(&reference.balances[0])), Lookup::Unknown));
            }
            ledger.apply(&clock(1), &[sample("9")]).unwrap();
            ledger.apply(&clock(2), &[]).unwrap();
            let reference = Events { balances: vec![sample("9")] };
            let c = compare(&empty, &reference, &ledger).unwrap();
            assert_eq!(c.retained_only_reference_matches, 1);
            assert_eq!(c.emitted_rows, 0);
            assert_eq!(c.cold_reference_observations, 0);
            assert!(compare(&empty, &Events { balances: vec![sample("8")] }, &ledger).is_err());
        }
    }
}
