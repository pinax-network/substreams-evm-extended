#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    offline::main()
}

#[cfg(not(target_arch = "wasm32"))]
mod offline {
    // Offline-only replay: no RPC client, endpoint, process runner or sink.
    use anyhow::{bail, ensure, Context, Result};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use evm_retention::{Clock, Key as RetainedKey, Ledger, Lookup};
    use prost::Message;
    use proto::pb::evm::balances::v1::{Balance, Events};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{self, File},
        io::{BufRead, BufReader, BufWriter, Write},
        path::{Path, PathBuf},
        time::Instant,
    };
    use substreams_ethereum::pb::eth::v2 as eth;

    const START: u64 = 122288006;
    const STOP: u64 = 122289030;
    type Key = (Option<Vec<u8>>, Vec<u8>);
    type Rows = BTreeMap<Key, String>;
    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }
    fn sha(path: &Path) -> Result<String> {
        Ok(digest(&fs::read(path)?))
    }
    fn read(path: &Path) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    fn write(path: &Path, value: &Value) -> Result<()> {
        fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
        Ok(())
    }
    fn binary(v: &Value, size: usize) -> Result<Vec<u8>> {
        let s = v.as_str().context("binary string")?;
        let bytes = if let Some(h) = s.strip_prefix("0x") {
            hex::decode(h)?
        } else {
            STANDARD.decode(s)?
        };
        ensure!(bytes.len() == size, "wrong binary size");
        Ok(bytes)
    }
    fn events(v: &Value) -> Result<Events> {
        let o = v.as_object().context("Events object")?;
        ensure!(o.keys().all(|k| k == "balances"), "unexpected Events field");
        let mut out = Events::default();
        if let Some(rows) = o.get("balances") {
            for row in rows.as_array().context("balances array")? {
                let r = row.as_object().context("Balance object")?;
                ensure!(
                    r.keys().all(|k| ["contract", "address", "amount"].contains(&k.as_str())),
                    "unexpected Balance field"
                );
                let amount = r.get("amount").and_then(Value::as_str).unwrap_or("").to_owned();
                ensure!(
                    !amount.is_empty() && amount.bytes().all(|b| b.is_ascii_digit()) && (amount == "0" || !amount.starts_with('0')),
                    "noncanonical amount"
                );
                let contract = r.get("contract").filter(|v| !v.is_null()).map(|v| binary(v, 20)).transpose()?;
                out.balances.push(Balance {
                    contract,
                    address: binary(&row["address"], 20)?,
                    amount,
                });
            }
        }
        normalize(&mut out)?;
        Ok(out)
    }
    fn normalize(events: &mut Events) -> Result<()> {
        events.balances.sort_by(|a, b| (&a.contract, &a.address).cmp(&(&b.contract, &b.address)));
        ensure!(
            events
                .balances
                .windows(2)
                .all(|w| (w[0].contract.as_ref(), &w[0].address) != (w[1].contract.as_ref(), &w[1].address)),
            "duplicate Balance key"
        );
        Ok(())
    }
    fn rows(events: &Events) -> Rows {
        events
            .balances
            .iter()
            .map(|b| ((b.contract.clone(), b.address.clone()), b.amount.clone()))
            .collect()
    }
    fn event_json(events: &Events) -> Value {
        json!({"balances": events.balances.iter().map(|b| json!({"contract": b.contract.as_ref().map(|c| format!("0x{}",hex::encode(c))), "address":format!("0x{}",hex::encode(&b.address)), "amount":b.amount})).collect::<Vec<_>>()})
    }
    fn key_json(k: &Key, amount: &str) -> Value {
        json!({"contract":k.0.as_ref().map(|c| format!("0x{}",hex::encode(c))),"address":format!("0x{}",hex::encode(&k.1)),"amount":amount})
    }
    fn stream(path: &Path) -> Result<BTreeMap<u64, Events>> {
        let mut result = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let v: Value = serde_json::from_str(&line?)?;
            ensure!(
                v["@module"] == "map_events" && v["@type"] == "evm.balances.v1.Events",
                "wrong output module or protobuf type"
            );
            let h = v["@block"].as_u64().context("block number")?;
            ensure!(
                (START..STOP).contains(&h) && result.insert(h, events(&v["@data"])?).is_none(),
                "duplicate or out-of-range output"
            );
        }
        ensure!(result.keys().copied().eq(START..STOP), "incomplete captured stream");
        Ok(result)
    }
    fn clocks(path: &Path) -> Result<BTreeMap<u64, String>> {
        let mut out = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let l = line?;
            let (h, tail) = l
                .strip_prefix("----------- BLOCK #")
                .context("clock prefix")?
                .split_once(" (")
                .context("clock height")?;
            let h: u64 = h.replace(',', "").parse()?;
            let (hash, suffix) = tail.split_once(") age=").context("clock hash")?;
            ensure!(suffix.ends_with(" ---------------") && hex::decode(hash)?.len() == 32, "clock format");
            ensure!(
                h == START + out.len() as u64 && out.insert(h, format!("0x{}", hash.to_ascii_lowercase())).is_none(),
                "clock gap or duplicate"
            );
        }
        ensure!(out.len() as u64 == STOP - START, "incomplete clocks");
        Ok(out)
    }
    fn source_files(dir: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let p = entry?.path();
            if p.is_dir() {
                source_files(&p, result)?;
            } else if p.extension().is_some_and(|e| e == "rs" || e == "proto") {
                result.push(p);
            }
        }
        Ok(())
    }
    fn source_inventory(root: &Path) -> Result<Value> {
        let repo = root.parent().unwrap().parent().unwrap();
        let mut paths = vec![
            repo.join("Cargo.toml"),
            repo.join("Cargo.lock"),
            repo.join("rust-toolchain.toml"),
            root.join("Cargo.toml"),
            repo.join("proto/Cargo.toml"),
        ];
        source_files(&root.join("src"), &mut paths)?;
        source_files(&repo.join("proto"), &mut paths)?;
        source_files(&root.join("tools/src"), &mut paths)?;
        source_files(&repo.join("common/retention/src"), &mut paths)?;
        paths.push(root.join("tools/Cargo.toml"));
        paths.push(repo.join("common/retention/Cargo.toml"));
        paths.push(root.join("tests/fixtures/role-path-candidates/layouts.json"));
        paths.push(root.join("tests/fixtures/role-path-candidates/source-review.json"));
        paths.sort();
        Ok(json!(paths
            .iter()
            .map(|p| Ok(json!({"path":p.strip_prefix(repo)?,"sha256":sha(p)?})))
            .collect::<Result<Vec<_>>>()?))
    }
    // Remove comments and quoted literals before counting identifier references.
    // This is a fail-closed check for these two pinned source bundles, not a
    // general Solidity call graph or proof for other deployments.
    fn identifiers(source: &str) -> Vec<String> {
        let bytes = source.as_bytes();
        let mut at = 0;
        let mut out = Vec::new();
        while at < bytes.len() {
            if bytes[at..].starts_with(b"//") {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
            } else if bytes[at..].starts_with(b"/*") {
                at += 2;
                while at < bytes.len() && !bytes[at..].starts_with(b"*/") {
                    at += 1;
                }
                at = (at + 2).min(bytes.len());
            } else if bytes[at] == b'\'' || bytes[at] == b'"' {
                let quote = bytes[at];
                at += 1;
                while at < bytes.len() && bytes[at] != quote {
                    if bytes[at] == b'\\' {
                        at += 1;
                    }
                    at += 1;
                }
                at = (at + 1).min(bytes.len());
            } else if bytes[at].is_ascii_alphabetic() || bytes[at] == b'_' {
                let start = at;
                at += 1;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
                out.push(source[start..at].to_owned());
            } else {
                at += 1;
            }
        }
        out
    }
    fn decode_hex(value: &Value) -> Result<Vec<u8>> {
        Ok(hex::decode(value.as_str().context("hex string")?.trim_start_matches("0x"))?)
    }
    fn verify_review(root: &Path, review: &Value, candidates: &Value) -> Result<Value> {
        ensure!(review["qualified"] == false, "candidate qualification flag");
        ensure!(
            sha(&root.join(review["baseline"]["path"].as_str().context("baseline path")?))? == review["baseline"]["sha256"],
            "baseline digest changed"
        );
        let audit_path = root.join(review["source_audit"]["path"].as_str().context("audit path")?);
        ensure!(sha(&audit_path)? == review["source_audit"]["sha256"], "source audit changed");
        let audit = read(&audit_path)?;
        let mut checks = Vec::new();
        let profiles = review["profiles"].as_array().context("review profiles")?;
        ensure!(profiles.len() == 2, "review scope");
        for profile in profiles {
            let contract = profile["contract"].as_str().context("contract")?;
            let candidate = candidates
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["contract"] == contract)
                .context("review candidate")?;
            let group = audit["groups"]
                .as_array()
                .context("audit groups")?
                .iter()
                .find(|g| g["profiles"].as_array().is_some_and(|ps| ps.iter().any(|p| p["contract"] == contract)))
                .context("audit group")?;
            ensure!(
                group["setter_reachability"] == "no_source_callsite" && group["source"] == profile["captured_source"],
                "source audit binding"
            );
            let source_path = root.join(profile["captured_source"]["path"].as_str().context("source path")?);
            ensure!(sha(&source_path)? == profile["captured_source"]["sha256"], "captured source digest changed");
            let captured = read(&source_path)?;
            ensure!(
                profile["storage_layout"] == captured["storageLayout"]
                    && profile["runtime"] == captured["runtimeBytecode"]
                    && profile["compilation"] == captured["compilation"],
                "source bundle changed"
            );
            let sources = profile["sources"].as_object().context("source files")?;
            ensure!(
                sources.len() == captured["sources"].as_object().context("captured sources")?.len(),
                "incomplete source bundle"
            );
            let mut setter_references = 0;
            for (name, source) in sources {
                let content = source["content"].as_str().context("source content")?;
                ensure!(
                    source["content"] == captured["sources"][name]["content"] && source["keccak256"] == captured["metadata"]["sources"][name]["keccak256"],
                    "source contents or hash changed: {name}"
                );
                ensure!(
                    format!("0x{}", hex::encode(erc20_balances::hash(content.as_bytes()))) == source["keccak256"],
                    "source hash failed: {name}"
                );
                let tokens = identifiers(content);
                setter_references += tokens.iter().filter(|t| t.as_str() == "_setRoleAdmin").count();
                if tokens.iter().any(|t| t == "_setRoleAdmin") {
                    ensure!(tokens.windows(2).any(|w| w == ["function", "_setRoleAdmin"]), "unexpected setter callsite");
                }
            }
            ensure!(setter_references == 1, "expected only the unused internal setter declaration");
            let runtime = &profile["runtime"];
            let mut compiled = decode_hex(&runtime["recompiledBytecode"])?;
            let onchain = decode_hex(&runtime["onchainBytecode"])?;
            let transformations = runtime["transformations"].as_array().context("transformations")?;
            for transform in transformations {
                ensure!(
                    transform["type"] == "replace" && transform["reason"] == "immutable",
                    "unreviewed runtime transformation"
                );
                let id = transform["id"].as_str().context("immutable id")?;
                let offset = transform["offset"].as_u64().context("immutable offset")? as usize;
                let replacement = decode_hex(&runtime["transformationValues"]["immutables"][id])?;
                ensure!(
                    runtime["immutableReferences"][id]
                        .as_array()
                        .context("immutable reference")?
                        .iter()
                        .any(|r| r["start"] == offset && r["length"] == replacement.len()),
                    "immutable reference mismatch"
                );
                let end = offset.checked_add(replacement.len()).context("immutable range overflow")?;
                ensure!(end <= compiled.len(), "immutable outside bytecode");
                compiled[offset..end].copy_from_slice(&replacement);
            }
            ensure!(compiled == onchain, "source compilation transformations do not reconstruct bound runtime");
            let code_hash = format!("0x{}", hex::encode(erc20_balances::hash(&onchain)));
            ensure!(
                candidate["code_hash"] == code_hash && group["effective_runtime_hash"] == code_hash,
                "runtime binding changed"
            );
            checks.push(json!({"contract":contract,"source_files":sources.len(),"all_source_hashes_match":true,"setter_references":setter_references,"setter_calls":0,"runtime_hash":code_hash,"runtime_reconstructed_from_compiled_bytes":true,"immutable_transformations":transformations.len(),"source_capture_sha256":profile["captured_source"]["sha256"]}));
        }
        Ok(json!(checks))
    }

    fn membership_writes(block: &eth::Block, candidates: &Value) -> Result<BTreeMap<String, u64>> {
        let mut images = BTreeMap::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            for (key, image) in &call.keccak_preimages {
                let key = hex::decode(key.trim_start_matches("0x"))?;
                let image = hex::decode(image.trim_start_matches("0x"))?;
                ensure!(erc20_balances::hash(&image).as_slice() == key, "invalid captured preimage");
                if let Some(previous) = images.insert(key, image.clone()) {
                    ensure!(previous == image, "conflicting preimage");
                }
            }
        }
        let mut counts = BTreeMap::new();
        for candidate in candidates.as_array().unwrap() {
            let contract = candidate["contract"].as_str().unwrap();
            let address = decode_hex(&candidate["contract"])?;
            let root = decode_hex(&candidate["other_mapping_paths"][0]["root"])?;
            let calls = block.system_calls.iter().chain(
                block
                    .transaction_traces
                    .iter()
                    .filter(|tx| tx.status() == eth::TransactionTraceStatus::Succeeded)
                    .flat_map(|tx| &tx.calls),
            );
            let mut count = 0;
            for write in calls
                .filter(|call| !call.state_reverted)
                .flat_map(|call| &call.storage_changes)
                .filter(|w| w.address == address)
            {
                if write
                    .old_value
                    .iter()
                    .skip_while(|b| **b == 0)
                    .eq(write.new_value.iter().skip_while(|b| **b == 0))
                {
                    continue;
                }
                if let Some(member) = images.get(&write.key).filter(|image| image.len() == 64 && image[..12] == [0; 12]) {
                    if images.get(&member[32..]).is_some_and(|role| role.len() == 64 && role[32..] == root) {
                        count += 1;
                    }
                }
            }
            counts.insert(contract.to_owned(), count);
        }
        Ok(counts)
    }

    fn run(root: &Path, output: &Path, report: &mut Value) -> Result<()> {
        let started = Instant::now();
        fs::copy(root.join("tools/src/bin/replay_role_candidates.rs"), output.join("replay-role-candidates.rs"))?;
        let fixture = root.join("tests/fixtures/bsc-refined450-layouts.json");
        let historical_path = root.join("out/refined450-combined/events.jsonl");
        let historical_report_path = root.join("out/refined450-combined/report.json");
        let clock_path = root.join("out/refined450-combined/events.clocks.txt");
        let reference_path = root.join("out/top50-1024/reference.jsonl");
        let reference_report_path = root.join("out/top50-1024/report.json");
        let hr = read(&historical_report_path)?;
        let rr = read(&reference_report_path)?;
        ensure!(hr["status"] == "bounded_parity", "historical report status");
        ensure!(
            hr["layouts_sha256"] == sha(&fixture)? && hr["events_sha256"] == sha(&historical_path)? && hr["clock_capture_sha256"] == sha(&clock_path)?,
            "historical binding mismatch"
        );
        ensure!(rr["reference_sha256"] == sha(&reference_path)?, "canonical reference binding mismatch");
        let baseline_text = fs::read_to_string(&fixture)?;
        let baseline = erc20_balances::layout::parse(&baseline_text).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let candidate_path = root.join("tests/fixtures/role-path-candidates/layouts.json");
        let review_path = root.join("tests/fixtures/role-path-candidates/source-review.json");
        let candidates = read(&candidate_path)?;
        let review = read(&review_path)?;
        let mut migrated: Value = serde_json::from_str(&baseline_text)?;
        ensure!(
            candidates.as_array().context("candidate array")?.len() == 2,
            "exactly two reviewed candidates required"
        );
        report["source_rechecks"] = verify_review(root, &review, &candidates)?;
        for candidate in candidates.as_array().unwrap() {
            let original = migrated
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|v| v["contract"] == candidate["contract"])
                .context("candidate absent from baseline")?;
            let path = candidate["other_mapping_paths"].as_array().context("typed paths")?;
            ensure!(
                path.len() == 1 && path[0]["key_types"] == json!(["bytes32", "address"]) && path[0]["offset"] == 0 && path[0]["words"] == 1,
                "candidate path changed"
            );
            let root = path[0]["root"].as_str().context("candidate root")?;
            let mut restored = candidate.clone();
            restored.as_object_mut().unwrap().remove("other_mapping_paths");
            ensure!(
                restored["other_mapping_words"]
                    .as_object_mut()
                    .context("legacy words")?
                    .insert(root.into(), json!(2))
                    .is_none(),
                "candidate retains broad role root"
            );
            ensure!(restored == *original, "candidate changes unrelated baseline fields");
            *original = candidate.clone();
        }
        write(&output.join("candidate-combined-layouts.json"), &migrated)?;
        let layouts = erc20_balances::layout::parse(&migrated.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        ensure!(layouts.len() == 431 && hr["configured_profiles"] == 431, "profile scope changed");
        let configured: BTreeSet<_> = layouts.iter().map(|l| l.contract.clone()).collect();
        let before = source_inventory(root)?;
        write(&output.join("source-inputs.json"), &before)?;
        report["inputs"] = json!([
            &fixture,
            &candidate_path,
            &review_path,
            &historical_path,
            &historical_report_path,
            &clock_path,
            &reference_path,
            &reference_report_path
        ]
        .iter()
        .map(|p| Ok(json!({"path":p,"sha256":sha(p)?})))
        .collect::<Result<Vec<_>>>()?);
        report["source_inventory_sha256"] = json!(sha(&output.join("source-inputs.json"))?);
        let historical = stream(&historical_path)?;
        let reference = stream(&reference_path)?;
        let clock = clocks(&clock_path)?;
        let mut native_file = BufWriter::new(File::create(output.join("native-events.jsonl"))?);
        let mut differences = BufWriter::new(File::create(output.join("differences.jsonl"))?);
        let mut inventory = BufWriter::new(File::create(output.join("blocks.jsonl"))?);
        let mut block_counts = BufWriter::new(File::create(output.join("per-block.jsonl"))?);
        let mut native_only = BufWriter::new(File::create(output.join("native-only-vs-reference.jsonl"))?);
        let mut reference_only = BufWriter::new(File::create(output.join("reference-only-configured.jsonl"))?);
        let mut ledger = Ledger::new(1);
        let mut selected_counts: BTreeMap<String, BTreeMap<&str, u64>> = candidates
            .as_array()
            .unwrap()
            .iter()
            .map(|v| (v["contract"].as_str().unwrap().to_owned(), BTreeMap::new()))
            .collect();
        let mut emitted_profiles = BTreeSet::new();
        let mut counts = BTreeMap::<&str, u64>::new();
        let mut prior = hr["first_parent_hash"].as_str().context("historical parent")?.to_owned();
        for height in START..STOP {
            let path = root.join(format!("out/top50-full-holder-blocks/{height}.pb"));
            let bytes = fs::read(&path)?;
            let block = eth::Block::decode(bytes.as_slice())?;
            let hash = format!("0x{}", hex::encode(&block.hash));
            let header = block.header.as_ref().context("missing block header")?;
            let parent = format!("0x{}", hex::encode(&header.parent_hash));
            ensure!(
                block.number == height && header.number == height && hash == clock[&height] && parent == prior,
                "cached block/clock identity mismatch at {height}"
            );
            ensure!(block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32, "non-Extended block");
            prior = hash.clone();
            for (contract, count) in membership_writes(&block, &candidates)? {
                *selected_counts
                    .get_mut(&contract)
                    .unwrap()
                    .entry("persisted_role_membership_writes")
                    .or_default() += count;
            }
            let mut actual = erc20_balances::project(&block, &layouts).map_err(|e| anyhow::anyhow!("native map failed at {height}: {e}"))?;
            normalize(&mut actual)?;
            let mut baseline_actual = erc20_balances::project(&block, &baseline).map_err(|e| anyhow::anyhow!("baseline map failed at {height}: {e}"))?;
            normalize(&mut baseline_actual)?;
            ensure!(actual == baseline_actual, "candidate differs from unchanged current baseline at {height}");
            ledger.apply(
                &Clock {
                    number: height,
                    hash: block.hash.clone(),
                    parent_hash: header.parent_hash.clone(),
                },
                &actual.balances,
            )?;
            if actual != historical[&height] {
                *counts.entry("historical_mismatch_blocks").or_default() += 1;
                writeln!(
                    differences,
                    "{}",
                    json!({"block":height,"kind":"historical_full_protobuf_mismatch","actual":event_json(&actual),"historical":event_json(&historical[&height])})
                )?;
            }
            let native_rows = rows(&actual);
            let reference_rows = rows(&reference[&height]);
            let mut same_block = 0;
            let mut extra = 0;
            let mut filtered_reference = 0;
            let mut carried = 0;
            let mut cold = 0;
            for (key, amount) in &native_rows {
                *counts.entry("native_rows").or_default() += 1;
                *counts.entry("native_zero_rows").or_default() += u64::from(amount == "0");
                emitted_profiles.insert(key.0.clone());
                if let Some(expected) = reference_rows.get(key) {
                    same_block += 1;
                    if amount != expected {
                        *counts.entry("canonical_rpc_mismatches").or_default() += 1;
                        writeln!(
                            differences,
                            "{}",
                            json!({"block":height,"kind":"canonical_same_block_mismatch","native":key_json(key,amount),"expected":expected})
                        )?;
                    }
                } else {
                    extra += 1;
                    writeln!(
                        native_only,
                        "{}",
                        json!({"block":height,"balance":key_json(key,amount),"historical_full_fields_matched":actual == historical[&height]})
                    )?;
                }
                if let Some(contract) = key.0.as_ref().map(|c| format!("0x{}", hex::encode(c))) {
                    if let Some(selected) = selected_counts.get_mut(&contract) {
                        *selected.entry("emitted_rows").or_default() += 1;
                        *selected.entry("same_block_reference_matches").or_default() += u64::from(reference_rows.get(key) == Some(amount));
                    }
                }
            }
            for (key, expected) in &reference_rows {
                *counts.entry("canonical_rpc_reference_rows_all_tokens").or_default() += 1;
                if !key.0.as_ref().is_some_and(|c| configured.contains(c)) {
                    continue;
                }
                filtered_reference += 1;
                if native_rows.contains_key(key) {
                    continue;
                }
                let retained = match ledger.lookup(&RetainedKey {
                    contract: key.0.clone(),
                    address: key.1.clone(),
                }) {
                    Lookup::Known(entry) => Some(&entry.value),
                    Lookup::Unknown => None,
                    other => bail!("unexpected retained classification: {other:?}"),
                };
                if let Some(contract) = key.0.as_ref().map(|c| format!("0x{}", hex::encode(c))) {
                    if let Some(selected) = selected_counts.get_mut(&contract) {
                        *selected
                            .entry(if retained.is_some() {
                                "retained_reference_matches"
                            } else {
                                "cold_reference_observations"
                            })
                            .or_default() += 1;
                        if retained.is_none() {
                            *selected.entry("cold_nonzero_reference_observations").or_default() += u64::from(expected != "0");
                        }
                    }
                }
                if let Some(amount) = retained {
                    carried += 1;
                    if amount != expected {
                        *counts.entry("canonical_retained_mismatches").or_default() += 1;
                        writeln!(
                            differences,
                            "{}",
                            json!({"block":height,"kind":"canonical_retained_mismatch","retained":key_json(key,amount),"expected":expected})
                        )?;
                    }
                } else {
                    cold += 1;
                }
                writeln!(
                    reference_only,
                    "{}",
                    json!({"block":height,"expected":key_json(key,expected),"native_known_amount":retained,"classification":if retained.is_some() {"retained_from_prior_native_emission"} else {"unknown_cold_holder_not_initialized"}})
                )?;
            }
            *counts.entry("blocks").or_default() += 1;
            *counts.entry("empty_native_blocks").or_default() += u64::from(native_rows.is_empty());
            *counts.entry("canonical_same_block_rows").or_default() += same_block;
            *counts.entry("native_only_vs_canonical_reference_rows").or_default() += extra;
            *counts.entry("canonical_configured_reference_rows").or_default() += filtered_reference;
            *counts.entry("canonical_reference_only_retained_matches").or_default() += carried;
            *counts.entry("canonical_reference_only_unknown_cold_holders").or_default() += cold;
            writeln!(
                native_file,
                "{}",
                json!({"@block":height,"@module":"map_events","@type":"evm.balances.v1.Events","@data":event_json(&actual)})
            )?;
            writeln!(
                inventory,
                "{}",
                json!({"block":height,"hash":hash,"parent_hash":parent,"path":path,"size":bytes.len(),"sha256":digest(&bytes),"native_protobuf_sha256":digest(&actual.encode_to_vec()),"historical_protobuf_sha256":digest(&historical[&height].encode_to_vec())})
            )?;
            writeln!(
                block_counts,
                "{}",
                json!({"block":height,"native_rows":native_rows.len(),"reference_rows_all":reference_rows.len(),"reference_rows_configured":filtered_reference,"same_block":same_block,"native_only":extra,"reference_only_retained":carried,"reference_only_unknown":cold})
            )?;
            if (height - START + 1) % 128 == 0 {
                eprintln!("Offline replay: {} / 1024 blocks", height - START + 1);
            }
        }
        for file in [
            &mut native_file,
            &mut differences,
            &mut inventory,
            &mut block_counts,
            &mut native_only,
            &mut reference_only,
        ] {
            file.flush()?;
        }
        report["counts"] = json!(counts);
        report["retention"] = json!(ledger.report());
        for (contract, selected) in &mut selected_counts {
            let address = hex::decode(contract.trim_start_matches("0x"))?;
            selected.insert(
                "initialized_observed_holders",
                ledger.entries().keys().filter(|k| k.contract.as_ref() == Some(&address)).count() as u64,
            );
        }
        report["candidate_profiles"] = json!(selected_counts);
        report["candidate_matches_current_baseline_all_blocks"] = json!(true);
        report["configured_profiles"] = json!(layouts.len());
        report["profiles_with_emitted_rows"] = json!(emitted_profiles.len());
        report["last_hash"] = json!(prior);
        report["first_hash"] = hr["first_hash"].clone();
        report["first_parent_hash"] = hr["first_parent_hash"].clone();
        report["all_1024_clocks_equal_saved_canonical_bound_clocks"] = json!(true);
        report["source_inputs_unchanged"] = json!(before == source_inventory(root)?);
        report["artifacts"] = json!([
            "native-events.jsonl",
            "candidate-combined-layouts.json",
            "replay-role-candidates.rs",
            "differences.jsonl",
            "blocks.jsonl",
            "per-block.jsonl",
            "native-only-vs-reference.jsonl",
            "reference-only-configured.jsonl"
        ]
        .iter()
        .map(|name| Ok(json!({"path":output.join(name),"sha256":sha(&output.join(name))?})))
        .collect::<Result<Vec<_>>>()?);
        report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
        ensure!(
            prior == hr["last_hash"].as_str().context("historical final hash")?,
            "last canonical block mismatch"
        );
        ensure!(report["source_inputs_unchanged"] == true, "mapper source changed while replaying");
        ensure!(
            counts.get("historical_mismatch_blocks").copied().unwrap_or(0) == 0
                && counts.get("canonical_rpc_mismatches").copied().unwrap_or(0) == 0
                && counts.get("canonical_retained_mismatches").copied().unwrap_or(0) == 0,
            "saved-data mismatches; see differences.jsonl"
        );
        report["status"] = json!("passed_offline_role_candidate_replay");
        Ok(())
    }
    pub fn main() -> Result<()> {
        let mut args = std::env::args().skip(1);
        let root = PathBuf::from(args.next().context("crate root required")?);
        let output = PathBuf::from(args.next().context("fresh output required")?);
        ensure!(args.next().is_none(), "unexpected arguments");
        fs::create_dir(&output).context("output must be fresh")?;
        let mut report = json!({"status":"incomplete","mode":"offline_saved_data_only","start_inclusive":START,"stop_exclusive":STOP,"network_requests":0,"new_rpc_balance_controls":0,"substreams_firehose_or_sink_commands":0,"comparison":"All current Events/Balance protobuf fields and optional contract presence; only balance row ordering normalized by (contract,address).","scope":"Two unqualified role-path candidates applied to an otherwise unchanged 431-profile baseline; native Rust replay, not a new WASM/package/live qualification. Historical output remains immutable. Canonical RPC capture is compared on overlap and on holders independently learned from native emissions; unknown cold holders are explicitly retained as unknown, with no hidden initial state."});
        if let Err(error) = run(&root, &output, &mut report) {
            report["status"] = json!("failed");
            report["error"] = json!(format!("{error:#}"));
            write(&output.join("report.json"), &report)?;
            bail!("{error:#}");
        }
        write(&output.join("report.json"), &report)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }
}
