//! Offline preparation of source-bound, unqualified issue-61 candidate evidence.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    host::run()
}

#[cfg(not(target_arch = "wasm32"))]
mod host {
    use anyhow::{ensure, Context};
    use erc20_balances::{hash, layout, project};
    use evm_retention::{Clock, Domain, Key, Ledger, Lookup};
    use primitive_types::U256;
    use prost::Message;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeMap,
        fs,
        io::{BufRead, BufReader},
        path::Path,
    };
    use substreams_ethereum::pb::eth::v2 as eth;

    fn sha(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }
    fn bytes(v: &Value) -> anyhow::Result<Vec<u8>> {
        Ok(hex::decode(v.as_str().context("hex string")?.trim_start_matches("0x"))?)
    }
    fn read(path: &Path) -> anyhow::Result<Value> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    fn save(dir: &Path, name: &str, value: &Value) -> anyhow::Result<()> {
        fs::write(dir.join(name), format!("{}\n", serde_json::to_string_pretty(value)?))?;
        Ok(())
    }
    fn source_identity(v: &Value, expected_address: &str) -> anyhow::Result<()> {
        ensure!(v["chainId"] == "56", "source capture is not BSC chain 56");
        let address = bytes(&v["address"])?;
        ensure!(
            address.len() == 20 && address == hex::decode(expected_address.trim_start_matches("0x"))?,
            "source capture address differs from expected contract/implementation"
        );
        Ok(())
    }
    fn replay_attempt(start: u64, stop: u64, mut visit: impl FnMut(u64) -> anyhow::Result<()>) -> (u64, Option<Value>) {
        for height in start..stop {
            if let Err(error) = visit(height) {
                return (height - start + 1, Some(json!({"block":height,"error":format!("{error:#}")})));
            }
        }
        (stop - start, None)
    }
    fn source(cache: &Path, relative: &str, expected_address: &str, expected: &str, capture_sha256: &str) -> anyhow::Result<Value> {
        let raw = fs::read(cache.join(relative))?;
        // Bind the complete reviewed compiler capture, including the full
        // source set and immutable schema, before trusting any of its fields.
        ensure!(sha(&raw) == capture_sha256, "source capture differs from reviewed SHA-256");
        let v: Value = serde_json::from_slice(&raw)?;
        source_identity(&v, expected_address)?;
        let sources = v["sources"].as_object().context("complete sources required")?;
        for (name, source) in sources {
            let text = source["content"].as_str().context("source contents")?;
            ensure!(
                format!("0x{}", hex::encode(hash(text.as_bytes()))) == v["metadata"]["sources"][name]["keccak256"],
                "source hash mismatch: {name}"
            );
        }
        let runtime = &v["runtimeBytecode"];
        let onchain = bytes(&runtime["onchainBytecode"])?;
        ensure!(format!("0x{}", hex::encode(hash(&onchain))) == expected, "bound runtime differs");
        let mut compiled = bytes(&runtime["recompiledBytecode"])?;
        for t in runtime["transformations"].as_array().context("transformations")? {
            ensure!(t["reason"] == "immutable" && t["type"] == "replace", "unreviewed bytecode transformation");
            let id = t["id"].as_str().context("immutable id")?;
            let offset = t["offset"].as_u64().context("immutable offset")? as usize;
            let replacement = bytes(&runtime["transformationValues"]["immutables"][id])?;
            ensure!(
                runtime["immutableReferences"][id]
                    .as_array()
                    .context("immutable references")?
                    .iter()
                    .any(|r| r["start"] == offset && r["length"] == replacement.len()),
                "undeclared immutable transformation"
            );
            compiled
                .get_mut(offset..offset + replacement.len())
                .context("immutable out of range")?
                .copy_from_slice(&replacement);
        }
        ensure!(compiled == onchain, "compiled source does not reconstruct bound runtime");
        Ok(
            json!({"capture":relative,"capture_sha256":sha(&raw),"qualified":false,"address":v["address"],"chain_id":56,"compiler":v["compilation"],"sources":v["sources"],"source_hashes":v["metadata"]["sources"],"runtime":runtime,"storage_layout":v["storageLayout"],"code_hash":expected,"source_reconstruction":"saved compiler bytecode plus declared immutable replacements, no fresh compilation"}),
        )
    }

    pub fn run() -> anyhow::Result<()> {
        let args: Vec<_> = std::env::args().collect();
        ensure!(
            args.len() == 3,
            "usage: review_bsc_exclusions <original erc20/balances directory> <fresh output directory>"
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().context("package root")?;
        let cache = Path::new(&args[1]).join("out");
        let output = Path::new(&args[2]);
        ensure!(!output.exists(), "output must be fresh");
        fs::create_dir_all(output)?;
        let baseline_bytes = fs::read(root.join("tests/fixtures/bsc-refined450-layouts.json"))?;
        ensure!(
            sha(&baseline_bytes) == "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468",
            "historical baseline changed"
        );
        let baseline: Vec<Value> = serde_json::from_slice(&baseline_bytes)?;
        let take = "0xe747e54783ba3f77a8e5251a3cba19ebe9c0e197";
        let radr = "0xf08d1886e2a69dfabd22a45eed8e1407b338b97e";
        let tops = "0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a";
        let guard = "0x9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f00";
        let original = baseline.iter().find(|p| p["contract"] == take).context("TAKE baseline")?;
        let mut candidate = original.clone();
        candidate["other_slots"].as_array_mut().context("other slots")?.push(json!(guard));
        save(output, "take-candidate-NOT-QUALIFIED.json", &json!([candidate]))?;
        save(
            output,
            "take-source.json",
            &source(
                &cache,
                "pending350-dependencies/0xa9709c4caf6e3fb5fd62b11b53b3f8b393c8927d-source.json",
                original["proxy"]["implementation"].as_str().unwrap(),
                original["proxy"]["code_hash"].as_str().unwrap(),
                "7efd25c828749c928cfbaf8217ff07f0349e5657b8421cd5e620cafe7af0a202",
            )?,
        )?;
        let tops_profile = baseline.iter().find(|p| p["contract"] == tops).unwrap();
        save(
            output,
            "tops-source.json",
            &source(
                &cache,
                "ranks201-250-source-review/0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a.json",
                tops,
                tops_profile["code_hash"].as_str().unwrap(),
                "bed5d7155189f849140fce44c4971a526a139b40b1ac4cc7d83db48b057a4c1a",
            )?,
        )?;
        let refusal = read(&root.join("docs/evidence/live-2026-09-23/refusal-scan.json"))?;
        let mut cases = Vec::new();
        for entry in refusal["refused"].as_array().context("refused list")? {
            let contract = entry["contract"].as_str().unwrap();
            let height = entry["block"].as_u64().unwrap();
            let input = cache.join(format!("live-2026-09-23/firehose-blocks/{height}.pb"));
            let raw = fs::read(&input)?;
            let block = eth::Block::decode(raw.as_slice())?;
            let profile = baseline.iter().find(|p| p["contract"] == contract).unwrap();
            let layouts = layout::parse(&json!([profile]).to_string())?;
            ensure!(
                project(&block, &layouts).unwrap_err().to_string() == entry["reason"],
                "original refusal changed"
            );
            let tx_hash = bytes(&entry["slot"]["writes"][0]["transaction"])?;
            let tx = block
                .transaction_traces
                .iter()
                .find(|tx| tx.hash == tx_hash)
                .context("refused transaction")?
                .clone();
            let trimmed = eth::Block {
                transaction_traces: vec![tx.clone()],
                system_calls: vec![],
                balance_changes: vec![],
                code_changes: vec![],
                ..block.clone()
            };
            ensure!(project(&trimmed, &layouts).unwrap_err().to_string() == entry["reason"], "trim lost refusal");
            let name = if contract == take {
                "take"
            } else if contract == radr {
                "radr"
            } else {
                "tops"
            };
            let fixture_name = format!("{name}-{height}-tx{}.pb", tx.index);
            // Protobuf maps have no canonical wire iteration order. Preserve
            // the first captured fixture bytes and prove their decoded content
            // equals the whole-transaction trim of the original cached block.
            let fixture = fs::read(root.join("tests/fixtures/bsc-exclusions-20260928").join(&fixture_name))?;
            ensure!(
                eth::Block::decode(fixture.as_slice())? == trimmed,
                "frozen fixture differs from captured transaction"
            );
            fs::write(output.join(&fixture_name), &fixture)?;
            let mut hints = BTreeMap::new();
            for call in &tx.calls {
                for (key, value) in &call.keccak_preimages {
                    let p = hex::decode(value.trim_start_matches("0x"))?;
                    let k = hex::decode(key.trim_start_matches("0x"))?;
                    ensure!(hash(&p).as_slice() == k, "bad preimage");
                    hints.insert(k, p);
                }
            }
            let mut dynamic_array_paths = Vec::new();
            let refused_key = bytes(&entry["slot"]["key"])?;
            for (head, p) in &hints {
                if p.len() == 64 && U256::from_big_endian(&p[32..]) == U256::from(31) {
                    let base = hash(head);
                    let (offset, _) = U256::from_big_endian(&refused_key).overflowing_sub(U256::from_big_endian(&base));
                    if offset < U256::from(1000000) {
                        dynamic_array_paths.push(json!({"root":31,"address":format!("0x{}",hex::encode(&p[12..32])),"head":format!("0x{}",hex::encode(head)),"array_base":format!("0x{}",hex::encode(base)),"word_offset":offset.low_u64(),"three_word_element_index":offset.low_u64()/3,"field_offset":offset.low_u64()%3}));
                    }
                }
            }
            let calls:Vec<_> = tx.calls.iter().filter(|c| c.storage_changes.iter().any(|w|w.key==refused_key)).map(|c|json!({"index":c.index,"address":format!("0x{}",hex::encode(&c.address)),"caller":format!("0x{}",hex::encode(&c.caller)),"input":format!("0x{}",hex::encode(&c.input)),"writes":c.storage_changes.iter().filter(|w|w.address==bytes(&entry["contract"]).unwrap()).map(|w|json!({"key":format!("0x{}",hex::encode(&w.key)),"old":format!("0x{}",hex::encode(&w.old_value)),"new":format!("0x{}",hex::encode(&w.new_value)),"ordinal":w.ordinal})).collect::<Vec<_>>()})).collect();
            cases.push(json!({"token":name,"contract":contract,"block":height,"hash":format!("0x{}",hex::encode(&block.hash)),"original_sha256":sha(&raw),"fixture":fixture_name,"fixture_sha256":sha(&fixture),"transaction":entry["slot"]["writes"][0]["transaction"],"transaction_index":tx.index,"original_refusal":entry["reason"],"refused_slot":entry["slot"]["key"],"calls":calls,"derived_lpinfo_paths":dynamic_array_paths,"layout":profile,"trim":"one whole transaction with original header; system and block effects omitted"}));
        }
        save(output, "cases.json", &json!(cases))?;
        let mut unresolved = Vec::new();
        for (token, contract, capture, getter, reason) in [
            (
                "RADR",
                radr,
                "ranks351-400-source-review/0xf08d1886e2a69dfabd22a45eed8e1407b338b97e.json",
                "ranks351-400-getters/f08d1886e2a69dfabd22a45eed8e1407b338b97e/report.json",
                "Mapping root 21 write has no bound source; no metadata permission added",
            ),
            (
                "BNC4",
                "0x7c8d5502b544ddaf8852fc46d1174e34876d545c",
                "candidate-sources-next/0x2781ba79f733ceda7467d2dd21688823d89ad53d.json",
                "bnc4-beacon-inspect/report.json",
                "Beacon implementation slot differs from old profile; new implementation/runtime unknown",
            ),
            (
                "sPro",
                "0xdfe1308fb3ef1dc2f87b4ffaef5ebdf80be1e4ea",
                "ranks101-150-source-review/0xdfe1308fb3ef1dc2f87b4ffaef5ebdf80be1e4ea.json",
                "spro-getter-review/report.json",
                "Balance divisor slot 11 differs; repinning cannot rebuild unchanged-holder balances",
            ),
            (
                "swkeyDAO2",
                "0x009b797edf9acc666a36020c4509c6095de51408",
                "ranks201-250-source-review/0x009b797edf9acc666a36020c4509c6095de51408.json",
                "swkey-divisor-review/report.json",
                "Balance divisor slot 9 differs; proxy source is not implementation source, and repinning cannot rebuild unchanged-holder balances",
            ),
        ] {
            let source_raw = fs::read(cache.join(capture))?;
            let source: Value = serde_json::from_slice(&source_raw)?;
            let getter_raw = fs::read(cache.join(getter))?;
            let getter_report: Value = serde_json::from_slice(&getter_raw)?;
            unresolved.push(json!({"token":token,"contract":contract,"status":"still_excluded","reason":reason,"source_capture":capture,"source_capture_sha256":sha(&source_raw),"source_match":source["match"],"source_compilation":source["compilation"],"getter_capture":getter,"getter_capture_sha256":sha(&getter_raw),"historical_getter":getter_report,"layout":baseline.iter().find(|p|p["contract"]==contract).unwrap()}));
        }
        save(output, "unresolved.json", &json!(unresolved))?;
        let candidate_layout = layout::parse(&json!([candidate]).to_string())?;
        let reference_file = cache.join("live-2026-09-23/ranking/reference.jsonl");
        ensure!(
            sha(&fs::read(&reference_file)?) == "0b9c49491f7ed4e698045aabc368bc668ff64cb3ff47618cef56897ecc5c7d30",
            "saved canonical reference changed"
        );
        let mut reference = BTreeMap::new();
        for line in BufReader::new(fs::File::open(&reference_file)?).lines() {
            let value: Value = serde_json::from_str(&line?)?;
            ensure!(
                value["@module"] == "map_events" && value["@type"] == "evm.balances.v1.Events",
                "wrong reference module/schema"
            );
            let prior = reference.insert(
                value["@block"].as_u64().context("reference clock")?,
                value["@data"]["balances"]
                    .as_array()
                    .context("reference balances")?
                    .iter()
                    .filter(|r| r["contract"] == take)
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            ensure!(prior.is_none(), "duplicate reference clock");
        }
        ensure!(reference.keys().copied().eq(123561000..123562024), "incomplete reference capture");
        let clocks_file = cache.join("live-2026-09-23/ranking/reference.clocks.txt");
        let mut clocks = BTreeMap::new();
        for line in BufReader::new(fs::File::open(&clocks_file)?).lines() {
            let line = line?;
            let (height, tail) = line
                .strip_prefix("----------- BLOCK #")
                .context("clock prefix")?
                .split_once(" (")
                .context("clock height")?;
            let height: u64 = height.replace(',', "").parse()?;
            let hash = hex::decode(tail.split(')').next().context("clock hash")?)?;
            ensure!(hash.len() == 32 && clocks.insert(height, hash).is_none(), "invalid/duplicate clock");
        }
        ensure!(clocks.keys().copied().eq(123561000..123562024), "incomplete canonical clocks");
        let mut rows = 0;
        let mut matches = 0;
        let mut missing = 0;
        let mut refusals = Vec::new();
        let mut previous = None;
        let mut outputs = Vec::new();
        let mut inventory = Vec::new();
        let mut ledger = Ledger::new(1, Domain::Balances);
        let mut known = 0;
        let mut cold = 0;
        let mut cold_nonzero = 0;
        let mut zeros = 0;
        let mut clock_hashes_checked = 0;
        let (attempted_blocks, failure) = replay_attempt(123561000, 123562024, |height| {
            let raw = fs::read(cache.join(format!("live-2026-09-23/firehose-blocks/{height}.pb")))?;
            let block = eth::Block::decode(raw.as_slice())?;
            inventory.push(json!({"block":height,"hash":format!("0x{}",hex::encode(&block.hash)),"sha256":sha(&raw)}));
            ensure!(block.number == height && block.hash == clocks[&height], "wrong height/hash");
            clock_hashes_checked += 1;
            if let Some(hash) = previous.as_ref() {
                ensure!(&block.header.as_ref().context("header")?.parent_hash == hash, "broken clock chain");
            }
            previous = Some(block.hash.clone());
            match project(&block, &candidate_layout) {
                Ok(events) => {
                    ledger.apply(
                        &Clock {
                            number: height,
                            hash: block.hash.clone(),
                            parent_hash: block.header.as_ref().context("header")?.parent_hash.clone(),
                        },
                        &events.balances,
                    )?;
                    for reference in &reference[&height] {
                        match ledger.lookup(&Key {
                            contract: Some(bytes(&reference["contract"])?),
                            address: bytes(&reference["address"])?,
                        }) {
                            Lookup::Known(value) => {
                                ensure!(value.value == reference["amount"].as_str().context("amount")?, "retained amount mismatch");
                                known += 1;
                            }
                            Lookup::Unknown => {
                                cold += 1;
                                cold_nonzero += usize::from(reference["amount"] != "0");
                            }
                            other => anyhow::bail!("unexpected retained state {other:?}"),
                        }
                    }
                    for row in events.balances {
                        let r = json!({"contract":format!("0x{}",hex::encode(row.contract.context("token")?)),"address":format!("0x{}",hex::encode(row.address)),"amount":row.amount});
                        rows += 1;
                        zeros += usize::from(r["amount"] == "0");
                        let same = reference
                            .get(&height)
                            .context("missing reference block")?
                            .iter()
                            .find(|r2| r2["address"] == r["address"]);
                        outputs.push(json!({"block":height,"hash":format!("0x{}",hex::encode(&block.hash)),"row":r,"same_block_rpc_match":same==Some(&r)}));
                        if let Some(expected) = same {
                            ensure!(expected == &r, "same-block mismatch");
                            matches += 1;
                        } else {
                            missing += 1;
                            anyhow::bail!("emitted row has no same-block canonical reference");
                        }
                    }
                }
                Err(error) => {
                    refusals.push(json!({"block":height,"error":error.to_string()}));
                    anyhow::bail!("mapper refused: {error}");
                }
            }
            Ok(())
        });
        save(output, "take-emitted-rows.json", &json!(outputs))?;
        save(output, "blocks.json", &json!(inventory))?;
        let mut bindings = BTreeMap::new();
        for name in [
            "take-candidate-NOT-QUALIFIED.json",
            "take-source.json",
            "tops-source.json",
            "cases.json",
            "unresolved.json",
        ] {
            bindings.insert(name, sha(&fs::read(output.join(name))?));
        }
        let mut implementation = BTreeMap::new();
        for name in ["tools/src/bin/review_bsc_exclusions.rs", "src/lib.rs", "src/persist.rs", "src/layout.rs"] {
            implementation.insert(name, sha(&fs::read(root.join(name))?));
        }
        save(
            output,
            "report.json",
            &json!({"mode":"offline_saved_data_only","qualified":false,"failure":failure,"baseline_sha256":sha(&baseline_bytes),"artifact_sha256":bindings,"implementation_sha256":implementation,"interval":{"start":123561000,"stop_exclusive":123562024,"blocks":inventory.len(),"attempted_blocks":attempted_blocks,"requested_blocks":1024,"complete":failure.is_none(),"all_hashes_equal_saved_canonical_clocks":clock_hashes_checked==inventory.len()},"candidate":"take-candidate-NOT-QUALIFIED.json","native_rows":rows,"emitted_zeros":zeros,"same_block_saved_rpc_matches":matches,"rows_without_same_block_reference":missing,"refusals":refusals,"retention":{"ledger":ledger.report(),"known_reference_matches":known,"cold_reference_observations":cold,"cold_nonzero_reference_observations":cold_nonzero,"initialization":"emitted rows only; no checkpoint or reference seeding"},"network_requests":0,"reference_sha256":sha(&fs::read(reference_file)?),"clocks_sha256":sha(&fs::read(clocks_file)?),"blocks_inventory_sha256":sha(&fs::read(output.join("blocks.json"))?),"emitted_rows_sha256":sha(&fs::read(output.join("take-emitted-rows.json"))?),"limitations":["No fresh compiler, runtime, RPC, Firehose, packaged WASM, sink or holder qualification","TAKE candidate adds one source-bound guard word only; other five profiles remain excluded"]}),
        )?;
        ensure!(
            failure.is_none(),
            "offline replay failed; partial report and outputs saved in {}",
            output.display()
        );
        println!("Offline issue-61 preparation: {rows} TAKE rows; {matches} saved-reference matches; {missing} without same-block reference.");
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn source_capture_pin_rejects_empty_sources_and_altered_immutable_schema() {
            let reduced: Value = serde_json::from_str(include_str!("../../../tests/fixtures/bsc-exclusions-20260928/take-source.json")).unwrap();
            let captured = json!({"address":reduced["address"],"chainId":"56","sources":reduced["sources"],"metadata":{"sources":reduced["source_hashes"]},"runtimeBytecode":reduced["runtime"],"compilation":reduced["compiler"],"storageLayout":reduced["storage_layout"]});
            let dir = tempfile::tempdir().unwrap();
            save(dir.path(), "capture.json", &captured).unwrap();
            let pin = sha(&fs::read(dir.path().join("capture.json")).unwrap());
            let address = "0xa9709c4caf6e3fb5fd62b11b53b3f8b393c8927d";
            let runtime = reduced["code_hash"].as_str().unwrap();
            source(dir.path(), "capture.json", address, runtime, &pin).unwrap();
            for field in ["sources", "immutables"] {
                let mut altered = captured.clone();
                if field == "sources" {
                    altered["sources"] = json!({});
                } else {
                    altered["runtimeBytecode"]["transformations"][0]["offset"] = json!(0);
                }
                save(dir.path(), "capture.json", &altered).unwrap();
                assert!(source(dir.path(), "capture.json", address, runtime, &pin)
                    .unwrap_err()
                    .to_string()
                    .contains("reviewed SHA-256"));
            }
        }

        #[test]
        fn replay_stops_on_first_failure_including_the_last_block() {
            for failed_height in [10, 11, 12] {
                for reason in ["mapper refused", "missing canonical reference"] {
                    let mut visited = Vec::new();
                    let mut applied = Vec::new();
                    let (attempted, failure) = replay_attempt(10, 13, |height| {
                        visited.push(height);
                        ensure!(height != failed_height, reason);
                        applied.push(height);
                        Ok(())
                    });
                    assert_eq!(attempted, failed_height - 10 + 1);
                    assert_eq!(visited, (10..=failed_height).collect::<Vec<_>>());
                    assert_eq!(applied, (10..failed_height).collect::<Vec<_>>());
                    assert_eq!(failure.unwrap(), json!({"block":failed_height,"error":reason}));
                }
            }
        }

        #[test]
        fn source_identity_rejects_wrong_network_address_and_missing_provenance() {
            let expected = "0xa9709c4caf6e3fb5fd62b11b53b3f8b393c8927d";
            let captured = json!({"chainId":"56","address":"0xa9709c4CaF6e3FB5fD62B11B53B3f8B393c8927d"});
            source_identity(&captured, expected).unwrap();
            for altered in [
                json!({"chainId":"1","address":expected}),
                json!({"chainId":"56","address":"0xe747e54783ba3f77a8e5251a3cba19ebe9c0e197"}),
                json!({"address":expected}),
                json!({"chainId":"56"}),
            ] {
                assert!(source_identity(&altered, expected).is_err());
            }
        }
    }
}
