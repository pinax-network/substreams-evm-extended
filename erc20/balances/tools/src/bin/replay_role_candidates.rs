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
    use evm_retention::{Clock, Domain, Key as RetainedKey, Ledger, Lookup};
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
    fn verify_reference_binding(hr: &Value, rr: &Value, reference_sha: &str, clock: &BTreeMap<u64, String>) -> Result<()> {
        ensure!(
            rr["chain_id"] == 56 && rr["start"] == START && rr["stop_exclusive"] == STOP && rr["blocks"] == STOP - START,
            "canonical reference network/interval mismatch"
        );
        ensure!(rr["reference_sha256"] == reference_sha, "canonical reference file binding mismatch");
        let bindings = hr["baseline_bindings"].as_array().context("historical baseline bindings")?;
        let references: Vec<_> = bindings.iter().filter(|b| b["path"] == "out/top50-1024/reference.jsonl").collect();
        ensure!(
            references.len() == 1 && references[0]["sha256"] == reference_sha,
            "canonical reference differs from historical baseline binding"
        );
        ensure!(
            hr["blocks"] == STOP - START
                && clock.keys().copied().eq(START..STOP)
                && rr["first_hash"] == hr["first_hash"]
                && rr["last_hash"] == hr["last_hash"]
                && hr["first_hash"].as_str() == clock.get(&START).map(String::as_str)
                && hr["last_hash"].as_str() == clock.get(&(STOP - 1)).map(String::as_str),
            "canonical reference fork/clock boundary mismatch"
        );
        Ok(())
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
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        TokenCys,
        Burnmint,
        PointBedrock,
        FheB2,
        Bas,
    }
    fn source_inventory(root: &Path, mode: Mode) -> Result<Value> {
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
        if mode == Mode::Bas {
            for name in ["layouts.json", "source-review.json", "source-capture.json", "primary-sources.json"] {
                paths.push(root.join(erc20_balances_tools::bas_role::FIXTURE).join(name));
            }
        } else if mode == Mode::FheB2 {
            for name in ["layouts.json", "source-review.json", "FHE.json", "B2Token.json", "primary-sources.json"] {
                paths.push(root.join(erc20_balances_tools::fhe_b2_roles::FIXTURE).join(name));
            }
        } else if mode == Mode::PointBedrock {
            for name in ["layouts.json", "source-review.json", "Point.json", "Bedrock.json", "primary-sources.json"] {
                paths.push(root.join(erc20_balances_tools::point_bedrock_roles::FIXTURE).join(name));
            }
        } else if mode == Mode::Burnmint {
            for name in ["layouts.json", "source-capture.json", "primary-sources.json"] {
                paths.push(root.join("tests/fixtures/burnmint-role-candidate").join(name));
            }
        } else {
            paths.push(root.join("tests/fixtures/role-path-candidates/layouts.json"));
            paths.push(root.join("tests/fixtures/role-path-candidates/source-review.json"));
        }
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

    fn bas_admin_writes(block: &eth::Block) -> u64 {
        use erc20_balances_tools::bas_role as bound;
        let address = hex::decode(&bound::CONTRACT[2..]).unwrap();
        let key = hex::decode(&bound::ADMIN_WORD[2..]).unwrap();
        block
            .system_calls
            .iter()
            .chain(
                block
                    .transaction_traces
                    .iter()
                    .filter(|tx| tx.status() == eth::TransactionTraceStatus::Succeeded)
                    .flat_map(|tx| &tx.calls),
            )
            .filter(|call| !call.state_reverted)
            .flat_map(|call| &call.storage_changes)
            .filter(|w| w.address == address && w.key == key)
            .filter(|w| !w.old_value.iter().skip_while(|b| **b == 0).eq(w.new_value.iter().skip_while(|b| **b == 0)))
            .count() as u64
    }

    fn run(root: &Path, cache: &Path, output: &Path, mode: Mode, report: &mut Value) -> Result<()> {
        let started = Instant::now();
        fs::copy(root.join("tools/src/bin/replay_role_candidates.rs"), output.join("replay-role-candidates.rs"))?;
        let fixture = root.join("tests/fixtures/bsc-refined450-layouts.json");
        let historical_path = cache.join("out/refined450-combined/events.jsonl");
        let historical_report_path = cache.join("out/refined450-combined/report.json");
        let clock_path = cache.join("out/refined450-combined/events.clocks.txt");
        let reference_path = cache.join("out/top50-1024/reference.jsonl");
        let reference_report_path = cache.join("out/top50-1024/report.json");
        let hr = read(&historical_report_path)?;
        let rr = read(&reference_report_path)?;
        ensure!(hr["status"] == "bounded_parity", "historical report status");
        ensure!(
            hr["layouts_sha256"] == sha(&fixture)? && hr["events_sha256"] == sha(&historical_path)? && hr["clock_capture_sha256"] == sha(&clock_path)?,
            "historical binding mismatch"
        );
        let clock = clocks(&clock_path)?;
        verify_reference_binding(&hr, &rr, &sha(&reference_path)?, &clock)?;
        report["canonical_reference_interval_and_baseline_binding_verified"] = json!(true);
        let baseline_text = fs::read_to_string(&fixture)?;
        let baseline = erc20_balances::layout::parse(&baseline_text).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let candidate_path = root.join(if mode == Mode::Bas {
            "tests/fixtures/bas-role-candidate/layouts.json"
        } else if mode == Mode::FheB2 {
            "tests/fixtures/fhe-b2-role-candidates/layouts.json"
        } else if mode == Mode::PointBedrock {
            "tests/fixtures/point-bedrock-role-candidates/layouts.json"
        } else if mode == Mode::Burnmint {
            "tests/fixtures/burnmint-role-candidate/layouts.json"
        } else {
            "tests/fixtures/role-path-candidates/layouts.json"
        });
        let review_path = root.join(if mode == Mode::Bas {
            "tests/fixtures/bas-role-candidate/source-review.json"
        } else if mode == Mode::FheB2 {
            "tests/fixtures/fhe-b2-role-candidates/source-review.json"
        } else if mode == Mode::PointBedrock {
            "tests/fixtures/point-bedrock-role-candidates/source-review.json"
        } else if mode == Mode::Burnmint {
            "tests/fixtures/burnmint-role-candidate/primary-sources.json"
        } else {
            "tests/fixtures/role-path-candidates/source-review.json"
        });
        let candidates = read(&candidate_path)?;
        let review = read(&review_path)?;
        let mut migrated: Value = serde_json::from_str(&baseline_text)?;
        ensure!(
            candidates.as_array().context("candidate array")?.len() == if matches!(mode, Mode::Burnmint | Mode::Bas) { 1 } else { 2 },
            "exact reviewed candidate scope required"
        );
        report["source_rechecks"] = if mode == Mode::Bas {
            use erc20_balances_tools::bas_role as bound;
            let raw = fs::read(root.join(bound::FIXTURE).join("source-capture.json"))?;
            ensure!(
                raw == fs::read(cache.join(format!("out/ranks101-150-source-review/{}.json", bound::CONTRACT)))?,
                "committed BAS capture differs from original cache"
            );
            let capture = bound::verify_capture(&raw)?;
            ensure!(review == bound::review(&capture)?, "BAS source review differs from complete capture");
            bound::verify_primary(&capture, &read(&root.join(bound::FIXTURE).join("primary-sources.json"))?)?;
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([review])
        } else if mode == Mode::FheB2 {
            use erc20_balances_tools::fhe_b2_roles as bound;
            let mut captures = Vec::new();
            for p in &bound::PROFILES {
                let raw = fs::read(root.join(bound::FIXTURE).join(format!("{}.json", p.name)))?;
                ensure!(raw == fs::read(cache.join(p.cache))?, "committed capture differs from original cache");
                captures.push(bound::verify_capture(&raw, p)?);
            }
            ensure!(review == bound::review(&captures)?, "source review differs from complete verified captures");
            bound::verify_primary(&captures, &read(&root.join(bound::FIXTURE).join("primary-sources.json"))?)?;
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            review["profiles"].clone()
        } else if mode == Mode::PointBedrock {
            use erc20_balances_tools::point_bedrock_roles as bound;
            let mut captures = Vec::new();
            for p in &bound::PROFILES {
                let raw = fs::read(root.join(bound::FIXTURE).join(format!("{}.json", p.name)))?;
                ensure!(
                    raw == fs::read(cache.join(format!("out/ranks251-300-source-review/{}.json", p.contract)))?,
                    "committed capture differs from original cache"
                );
                captures.push(bound::verify_capture(&raw, p)?);
            }
            ensure!(review == bound::review(&captures)?, "source review differs from verified complete captures");
            bound::verify_primary(&captures, &read(&root.join(bound::FIXTURE).join("primary-sources.json"))?)?;
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            review["profiles"].clone()
        } else if mode == Mode::Burnmint {
            use erc20_balances_tools::burnmint_role as bound;
            let raw = fs::read(root.join("tests/fixtures/burnmint-role-candidate/source-capture.json"))?;
            let original = fs::read(cache.join(format!("out/ranks201-250-source-review/{}.json", bound::CONTRACT)))?;
            ensure!(raw == original, "committed source capture differs from original cache");
            let capture = bound::verify_capture(&raw)?;
            bound::verify_primary(&capture, &review)?;
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([{"contract":bound::CONTRACT,"chain_id":56,"qualified":false,"source_capture_sha256":bound::CAPTURE,"source_files":14,"runtime_hash":bound::RUNTIME,"immutable_transformations":3,"runtime_reconstructed_from_saved_compiler_bytes":true,"primary_pin":bound::PIN,"primary_import_relocations":1}])
        } else {
            verify_review(root, &review, &candidates)?
        };
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
            if mode == Mode::Bas {
                let slots = restored["other_slots"].as_array_mut().context("BAS scalar slots")?;
                ensure!(
                    slots.pop() == Some(json!(erc20_balances_tools::bas_role::ADMIN_WORD)),
                    "only the appended fixed BAS admin word may be removed"
                );
            }
            ensure!(
                restored["other_mapping_words"]
                    .as_object_mut()
                    .context("legacy words")?
                    .insert(root.into(), json!(2))
                    .is_none(),
                "candidate retains broad role root"
            );
            // Only FHE originally had this role root in BOTH legacy lists.
            // Its source-bound candidate verifier above requires removing both.
            if mode == Mode::FheB2 && candidate["contract"] == erc20_balances_tools::fhe_b2_roles::PROFILES[0].contract {
                let broad = restored["other_mapping_slots"].as_array_mut().context("FHE legacy mapping list")?;
                ensure!(
                    broad.as_slice()
                        == [
                            json!(erc20_balances_tools::fhe_b2_roles::root(1)),
                            json!(erc20_balances_tools::fhe_b2_roles::root(8))
                        ],
                    "FHE unrelated legacy mappings changed"
                );
                broad.insert(1, json!(root));
            }
            ensure!(restored == *original, "candidate changes unrelated baseline fields");
            *original = candidate.clone();
        }
        write(&output.join("candidate-combined-layouts.json"), &migrated)?;
        let layouts = erc20_balances::layout::parse(&migrated.to_string()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        ensure!(layouts.len() == 431 && hr["configured_profiles"] == 431, "profile scope changed");
        let configured: BTreeSet<_> = layouts.iter().map(|l| l.contract.clone()).collect();
        let before = source_inventory(root, mode)?;
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
        let mut native_file = BufWriter::new(File::create(output.join("native-events.jsonl"))?);
        let mut differences = BufWriter::new(File::create(output.join("differences.jsonl"))?);
        let mut inventory = BufWriter::new(File::create(output.join("blocks.jsonl"))?);
        let mut block_counts = BufWriter::new(File::create(output.join("per-block.jsonl"))?);
        let mut native_only = BufWriter::new(File::create(output.join("native-only-vs-reference.jsonl"))?);
        let mut reference_only = BufWriter::new(File::create(output.join("reference-only-configured.jsonl"))?);
        let mut ledger = Ledger::new(1, Domain::Balances);
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
            report["attempted_block"] = json!(height);
            let path = cache.join(format!("out/top50-full-holder-blocks/{height}.pb"));
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
            if mode == Mode::Bas {
                *selected_counts
                    .get_mut(erc20_balances_tools::bas_role::CONTRACT)
                    .unwrap()
                    .entry("persisted_fixed_pauser_admin_writes")
                    .or_default() += bas_admin_writes(&block);
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
            // Preserve progress if a later mapper/identity/ledger check fails.
            // Never insert fake empty rows to bridge a refusal.
            report["counts"] = json!(counts);
            report["candidate_profiles"] = json!(selected_counts);
            report["retention"] = json!(ledger.report());
            report["last_completed_block"] = json!(height);
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
        report["source_inputs_unchanged"] = json!(before == source_inventory(root, mode)?);
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
        let mode = match args.next().as_deref() {
            None => Mode::TokenCys,
            Some("--burnmint") => Mode::Burnmint,
            Some("--point-bedrock") => Mode::PointBedrock,
            Some("--fhe-b2") => Mode::FheB2,
            Some("--bas") => Mode::Bas,
            _ => bail!("expected --burnmint, --point-bedrock, --fhe-b2 or --bas, followed by <original package root>"),
        };
        let cache = if mode != Mode::TokenCys {
            PathBuf::from(args.next().context("original package root required")?)
        } else {
            root.clone()
        };
        ensure!(args.next().is_none(), "unexpected arguments");
        fs::create_dir(&output).context("output must be fresh")?;
        let mut report = json!({"status":"incomplete","mode":"offline_saved_data_only","start_inclusive":START,"stop_exclusive":STOP,"network_requests":0,"new_rpc_balance_controls":0,"substreams_firehose_or_sink_commands":0,"comparison":"All current Events/Balance protobuf fields and optional contract presence; only balance row ordering normalized by (contract,address).","scope":"Two unqualified role-path candidates applied to an otherwise unchanged 431-profile baseline; native Rust replay, not a new WASM/package/live qualification. Historical output remains immutable. Canonical RPC capture is compared on overlap and on holders independently learned from native emissions; unknown cold holders are explicitly retained as unknown, with no hidden initial state."});
        if mode == Mode::Burnmint {
            report["scope"] = json!("One unqualified BurnMint exact role path applied to unchanged 431; full saved native parity, not package/runtime/live qualification. Canonical values never seed retained state.");
        }
        if mode == Mode::PointBedrock {
            report["scope"] = json!("Two NOT-QUALIFIED Point/Bedrock membership paths applied to unchanged 431; saved native parity is not replacement package/runtime/live qualification. Point token primary repository remains unresolved. Canonical values never seed retained state.");
        }
        if mode == Mode::FheB2 {
            report["scope"]=json!("Two NOT-QUALIFIED FHE/B2Token exact membership paths applied to unchanged 431; exact saved immutable reconstruction and native parity are not replacement package/runtime/live qualification. Canonical values never seed retained state.");
        }
        if mode == Mode::Bas {
            report["scope"] = json!("One NOT-QUALIFIED BAS exact membership path plus one fixed PAUSER admin word applied to historical431; exact saved source/runtime/constructor reconstruction and native parity do not qualify deployment, current admin or replacement package. Public token source gap remains unresolved. Canonical values never seed retained state.");
        }
        if let Err(error) = run(&root, &cache, &output, mode, &mut report) {
            report["status"] = json!("failed");
            report["error"] = json!(format!("{error:#}"));
            write(&output.join("report.json"), &report)?;
            bail!("{error:#}");
        }
        write(&output.join("report.json"), &report)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn bas_fixed_admin_count_requires_exact_contract_slot_and_persisted_change() {
            use erc20_balances_tools::bas_role as bound;
            let call = eth::Call {
                storage_changes: vec![eth::StorageChange {
                    address: hex::decode(&bound::CONTRACT[2..]).unwrap(),
                    key: hex::decode(&bound::ADMIN_WORD[2..]).unwrap(),
                    old_value: vec![0],
                    new_value: vec![1],
                    ordinal: 1,
                }],
                ..Default::default()
            };
            let mut block = eth::Block {
                system_calls: vec![call.clone()],
                transaction_traces: vec![eth::TransactionTrace {
                    status: eth::TransactionTraceStatus::Succeeded as i32,
                    calls: vec![call],
                    ..Default::default()
                }],
                ..Default::default()
            };
            assert_eq!(bas_admin_writes(&block), 2);
            block.system_calls[0].state_reverted = true;
            assert_eq!(bas_admin_writes(&block), 1);
            for field in ["key", "address", "noop", "failed", "reverted"] {
                let mut b = block.clone();
                let tx = &mut b.transaction_traces[0];
                match field {
                    "key" => tx.calls[0].storage_changes[0].key[0] ^= 1,
                    "address" => tx.calls[0].storage_changes[0].address[0] ^= 1,
                    "noop" => tx.calls[0].storage_changes[0].old_value = vec![0, 1],
                    "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                    _ => tx.calls[0].state_reverted = true,
                }
                assert_eq!(bas_admin_writes(&b), 0, "{field}");
            }
        }

        #[test]
        fn canonical_reference_requires_original_network_interval_fork_and_baseline_binding() {
            let clock: BTreeMap<_, _> = (START..STOP).map(|h| (h, format!("0x{h:064x}"))).collect();
            let hr = json!({"blocks":STOP-START,"first_hash":clock[&START],"last_hash":clock[&(STOP-1)],"baseline_bindings":[{"path":"out/top50-1024/reference.jsonl","sha256":"original"}]});
            let rr = json!({"chain_id":56,"start":START,"stop_exclusive":STOP,"blocks":STOP-START,"reference_sha256":"original","first_hash":clock[&START],"last_hash":clock[&(STOP-1)]});
            verify_reference_binding(&hr, &rr, "original", &clock).unwrap();
            for (key, value) in [
                ("chain_id", json!(1)),
                ("start", json!(START + 1)),
                ("stop_exclusive", json!(STOP + 1)),
                ("blocks", json!(1)),
                ("first_hash", json!("different fork")),
                ("last_hash", json!("different fork")),
                ("reference_sha256", json!("replacement")),
            ] {
                let mut changed = rr.clone();
                changed[key] = value;
                assert!(verify_reference_binding(&hr, &changed, "original", &clock).is_err(), "{key}");
            }
            // A replacement file and self-consistent replacement report still
            // cannot overwrite the original report's canonical binding.
            let mut changed = rr.clone();
            changed["reference_sha256"] = json!("replacement");
            assert!(verify_reference_binding(&hr, &changed, "replacement", &clock).is_err());
            for bindings in [json!([]), json!([hr["baseline_bindings"][0], hr["baseline_bindings"][0]])] {
                let mut changed = hr.clone();
                changed["baseline_bindings"] = bindings;
                assert!(verify_reference_binding(&changed, &rr, "original", &clock).is_err());
            }
            let mut missing = clock.clone();
            missing.remove(&(START + 1));
            assert!(verify_reference_binding(&hr, &rr, "original", &missing).is_err());
            let mut changed = clock;
            changed.insert(START, "different fork".into());
            assert!(verify_reference_binding(&hr, &rr, "original", &changed).is_err());
        }
    }
}
