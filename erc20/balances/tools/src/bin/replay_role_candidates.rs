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
        Tagger,
        Artx,
        Oft,
        PToken,
        Securities,
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
        if mode == Mode::Securities {
            use erc20_balances_tools::securities_role as bound;
            for name in ["layouts.json", "source-review.json"] {
                paths.push(root.join(bound::FIXTURE).join(name));
            }
            for entry in fs::read_dir(root.join(bound::PROOF_FIXTURE))? {
                let path = entry?.path();
                if path.is_file() {
                    paths.push(path);
                }
            }
            for name in [
                "docs/evidence/securities-operation-proof-20260928.json",
                "docs/evidence/securities-operation-proof-20260928-transcripts.json",
                "docs/evidence/securities-operation-proof-20260928-compiler.json",
                "tests/securities_coupled_operations.rs",
                "tools/tests/securities_role_candidate.rs",
            ] {
                paths.push(root.join(name));
            }
        } else if mode == Mode::PToken {
            for name in ["layouts.json", "source-review.json"] {
                paths.push(root.join(erc20_balances_tools::ptoken_role::FIXTURE).join(name));
            }
            for name in ["capture.json", "compiler-output.json", "primary-sources.json", "LICENSE-OZ"] {
                paths.push(root.join(erc20_balances_tools::ptoken_role::PROOF_FIXTURE).join(name));
            }
            for name in [
                "docs/evidence/ptoken-operation-proof-20260928.json",
                "docs/evidence/ptoken-operation-proof-20260928-transcripts.json",
                "tests/ptoken_coupled_adversarial.rs",
                "tools/tests/ptoken_role_candidate.rs",
            ] {
                paths.push(root.join(name));
            }
        } else if mode == Mode::Oft {
            for name in [
                "layouts.json",
                "source-review.json",
                "kgen.json",
                "deep.json",
                "proxy.json",
                "primary-sources.json",
                "vendored-input.json",
            ] {
                paths.push(root.join(erc20_balances_tools::oft_roles::FIXTURE).join(name));
            }
        } else if mode == Mode::Artx {
            for name in [
                "layouts.json",
                "source-review.json",
                "proxy.json",
                "implementation.json",
                "primary-sources.json",
            ] {
                paths.push(root.join(erc20_balances_tools::artx_role::FIXTURE).join(name));
            }
        } else if mode == Mode::Tagger {
            for name in ["layouts.json", "source-review.json", "TaggerToken.json"] {
                paths.push(root.join(erc20_balances_tools::tagger_role::FIXTURE).join(name));
            }
        } else if mode == Mode::Bas {
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
            let coupled = candidate.get("enumerable_address_sets").is_some();
            let root = decode_hex(if coupled {
                &candidate["enumerable_address_sets"][0]["membership_root"]
            } else {
                &candidate["other_mapping_paths"][0]["root"]
            })?;
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
                if coupled && (write.key.len() != 32 || write.old_value.len() > 32 || write.new_value.len() > 32) {
                    continue;
                }
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

    // Count only changed persisted writes to the exact source-derived scalar.
    // Called after successful projection; this is not initializer execution proof.
    fn securities_admin_writes(block: &eth::Block, candidates: &Value) -> Result<BTreeMap<String, u64>> {
        let key = hex::decode(&erc20_balances_tools::securities_role::ADMIN_SLOT[2..])?;
        candidates
            .as_array()
            .context("Securities candidates")?
            .iter()
            .map(|candidate| {
                let contract = candidate["contract"].as_str().context("Securities contract")?;
                let address = decode_hex(&candidate["contract"])?;
                let count = block
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
                    .filter(|w| w.address == address && w.key == key && w.old_value.len() <= 32 && w.new_value.len() <= 32)
                    .filter(|w| w.new_value.iter().all(|b| *b == 0) && w.old_value.iter().any(|b| *b != 0))
                    .count() as u64;
                Ok((contract.to_owned(), count))
            })
            .collect()
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

    // Exact Deep initializer-derived locations only. This measures persisted
    // writes; it does not prove initializer context, values or authorization.
    fn deep_admin_writes(block: &eth::Block) -> u64 {
        use erc20_balances_tools::oft_roles as bound;
        let address = hex::decode(&bound::DEEP[2..]).unwrap();
        let keys: BTreeSet<_> = bound::admin_words().iter().map(|key| hex::decode(&key[2..]).unwrap()).collect();
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
            .filter(|w| w.address == address && keys.contains(&w.key) && w.old_value.len() <= 32 && w.new_value.len() <= 32)
            .filter(|w| !w.old_value.iter().skip_while(|b| **b == 0).eq(w.new_value.iter().skip_while(|b| **b == 0)))
            .count() as u64
    }

    // Tagger's public owner setter accepts arbitrary bytes32 role keys. This
    // counts source-shaped persisted storage writes, not executed authorization.
    fn tagger_admin_writes(block: &eth::Block) -> Result<u64> {
        use erc20_balances_tools::tagger_role as bound;
        let address = hex::decode(&bound::CONTRACT[2..])?;
        let root = hex::decode(&bound::root(6)[2..])?;
        let mut images = BTreeMap::new();
        let mut admin_words = BTreeSet::new();
        for call in block.system_calls.iter().chain(block.transaction_traces.iter().flat_map(|tx| &tx.calls)) {
            for (key, image) in &call.keccak_preimages {
                let key = hex::decode(key.trim_start_matches("0x"))?;
                let image = hex::decode(image.trim_start_matches("0x"))?;
                ensure!(erc20_balances::hash(&image).as_slice() == key, "invalid Tagger admin preimage");
                if let Some(previous) = images.insert(key.clone(), image.clone()) {
                    ensure!(previous == image, "conflicting Tagger admin preimage");
                }
                if image.len() == 64 && image[32..] == root {
                    // Solidity storage offsets use wrapping uint256 addition.
                    let mut admin = key;
                    for byte in admin.iter_mut().rev() {
                        let (next, carry) = byte.overflowing_add(1);
                        *byte = next;
                        if !carry {
                            break;
                        }
                    }
                    admin_words.insert(admin);
                }
            }
        }
        Ok(block
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
            .filter(|w| w.address == address && admin_words.contains(&w.key) && w.old_value.len() <= 32 && w.new_value.len() <= 32)
            .filter(|w| !w.old_value.iter().skip_while(|b| **b == 0).eq(w.new_value.iter().skip_while(|b| **b == 0)))
            .count() as u64)
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
        let candidate_path = root.join(if mode == Mode::Securities {
            "tests/fixtures/securities-coupled-role-candidate/layouts.json"
        } else if mode == Mode::PToken {
            "tests/fixtures/ptoken-coupled-role-candidate/layouts.json"
        } else if mode == Mode::Oft {
            "tests/fixtures/oft-role-candidates/layouts.json"
        } else if mode == Mode::Artx {
            "tests/fixtures/artx-role-candidate/layouts.json"
        } else if mode == Mode::Tagger {
            "tests/fixtures/tagger-role-candidate/layouts.json"
        } else if mode == Mode::Bas {
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
        let review_path = root.join(if mode == Mode::Securities {
            "tests/fixtures/securities-coupled-role-candidate/source-review.json"
        } else if mode == Mode::PToken {
            "tests/fixtures/ptoken-coupled-role-candidate/source-review.json"
        } else if mode == Mode::Oft {
            "tests/fixtures/oft-role-candidates/source-review.json"
        } else if mode == Mode::Artx {
            "tests/fixtures/artx-role-candidate/source-review.json"
        } else if mode == Mode::Tagger {
            "tests/fixtures/tagger-role-candidate/source-review.json"
        } else if mode == Mode::Bas {
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
            candidates.as_array().context("candidate array")?.len()
                == if mode == Mode::Securities {
                    17
                } else if matches!(mode, Mode::Burnmint | Mode::Bas | Mode::Tagger | Mode::Artx | Mode::PToken) {
                    1
                } else {
                    2
                },
            "exact reviewed candidate scope required"
        );
        report["source_rechecks"] = if mode == Mode::Securities {
            use erc20_balances_tools::securities_role as bound;
            let raw = fs::read(root.join(bound::PROOF_FIXTURE).join("capture.json"))?;
            ensure!(raw == fs::read(cache.join(bound::CACHE))?, "Securities capture differs from original cache");
            ensure!(
                review
                    == bound::review(
                        &raw,
                        &fs::read(root.join(bound::PROOF_FIXTURE).join("compiler-output.json"))?,
                        &fs::read(root.join("docs/evidence/securities-operation-proof-20260928-transcripts.json"))?,
                        &fs::read(root.join("docs/evidence/securities-operation-proof-20260928.json"))?,
                        &fs::read(root.join(bound::PROOF_FIXTURE).join("primary-sources.json"))?,
                        &fs::read(root.join("docs/evidence/securities-operation-proof-20260928-compiler.json"))?,
                    )?,
                "Securities review differs from complete frozen proof"
            );
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([review])
        } else if mode == Mode::PToken {
            use erc20_balances_tools::ptoken_role as bound;
            let raw = fs::read(root.join(bound::PROOF_FIXTURE).join("capture.json"))?;
            ensure!(raw == fs::read(cache.join(bound::CACHE))?, "PToken capture differs from original cache");
            ensure!(
                review
                    == bound::review(
                        &raw,
                        &fs::read(root.join(bound::PROOF_FIXTURE).join("compiler-output.json"))?,
                        &fs::read(root.join("docs/evidence/ptoken-operation-proof-20260928-transcripts.json"))?,
                        &fs::read(root.join("docs/evidence/ptoken-operation-proof-20260928.json"))?,
                        &fs::read(root.join(bound::PROOF_FIXTURE).join("primary-sources.json"))?,
                    )?,
                "PToken review differs from frozen complete proof"
            );
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([review])
        } else if mode == Mode::Oft {
            use erc20_balances_tools::oft_roles as bound;
            let mut captures = Vec::new();
            for p in &bound::CAPTURES {
                let raw = fs::read(root.join(bound::FIXTURE).join(format!("{}.json", p.label)))?;
                ensure!(raw == fs::read(cache.join(p.cache))?, "committed OFT capture differs from original cache");
                captures.push(bound::verify_capture(&raw, p)?);
            }
            ensure!(review == bound::review(&captures)?, "OFT review differs from complete captures");
            bound::verify_primary(
                &captures,
                &read(&root.join(bound::FIXTURE).join("primary-sources.json"))?,
                &fs::read(root.join(bound::FIXTURE).join("vendored-input.json"))?,
            )?;
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([review])
        } else if mode == Mode::Artx {
            use erc20_balances_tools::artx_role as bound;
            let mut captures = Vec::new();
            for p in &bound::CAPTURES {
                let raw = fs::read(root.join(bound::FIXTURE).join(format!("{}.json", p.label)))?;
                ensure!(raw == fs::read(cache.join(p.cache))?, "committed Artx capture differs from original cache");
                captures.push(bound::verify_capture(&raw, p)?);
            }
            ensure!(review == bound::review(&captures)?, "Artx review differs from complete captures");
            bound::verify_primary_raw(&fs::read(root.join(bound::FIXTURE).join("primary-sources.json"))?, &captures)?;
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([review])
        } else if mode == Mode::Tagger {
            use erc20_balances_tools::tagger_role as bound;
            let raw = fs::read(root.join(bound::FIXTURE).join("TaggerToken.json"))?;
            ensure!(
                raw == fs::read(cache.join(bound::CACHE))?,
                "committed Tagger capture differs from original cache"
            );
            let capture = bound::verify_capture(&raw)?;
            ensure!(review == bound::review(&capture)?, "Tagger review differs from complete capture");
            bound::verify_candidate(baseline_text.as_bytes(), &candidates)?;
            json!([review.clone()])
        } else if mode == Mode::Bas {
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
            if mode == Mode::Securities {
                use erc20_balances_tools::securities_role as bound;
                let mut restored = candidate.clone();
                ensure!(
                    restored.as_object_mut().unwrap().remove("enumerable_address_sets").is_some(),
                    "Securities coupled rule missing"
                );
                ensure!(
                    restored["other_mapping_words"]
                        .as_object_mut()
                        .context("Securities legacy words")?
                        .insert(bound::MEMBERSHIP_ROOT.to_owned(), json!(2))
                        .is_none(),
                    "Securities broad membership retained"
                );
                ensure!(
                    restored["other_slots"].as_array_mut().context("Securities scalar slots")?.pop() == Some(json!(bound::ADMIN_SLOT)),
                    "only exact appended ISSUER admin word"
                );
                ensure!(restored == *original, "Securities candidate changed unrelated fields");
                *original = candidate.clone();
                continue;
            }
            if mode == Mode::PToken {
                // Full exact-rule verification ran above. Restore only the removed
                // legacy membership permission, then compare every baseline field.
                let mut restored = candidate.clone();
                ensure!(
                    restored.as_object_mut().unwrap().remove("enumerable_address_sets").is_some(),
                    "coupled rule missing"
                );
                ensure!(
                    restored["other_mapping_words"]
                        .as_object_mut()
                        .context("legacy words")?
                        .insert(erc20_balances_tools::ptoken_role::root(5), json!(2))
                        .is_none(),
                    "broad membership retained"
                );
                ensure!(restored == *original, "PToken candidate changed unrelated fields");
                *original = candidate.clone();
                continue;
            }
            let path = candidate["other_mapping_paths"].as_array().context("typed paths")?;
            if mode == Mode::Tagger {
                ensure!(
                    candidate["other_mapping_paths"]
                        == json!([
                            {"root":erc20_balances_tools::tagger_role::root(6),"key_types":["bytes32"],"offset":1,"words":1},
                            {"root":erc20_balances_tools::tagger_role::root(6),"key_types":["bytes32","address"],"offset":0,"words":1}
                        ]),
                    "only Tagger's exact admin and membership paths"
                );
            } else {
                ensure!(
                    path.len() == 1 && path[0]["key_types"] == json!(["bytes32", "address"]) && path[0]["offset"] == 0 && path[0]["words"] == 1,
                    "candidate path changed"
                );
            }
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
            if mode == Mode::Oft && candidate["contract"] == erc20_balances_tools::oft_roles::DEEP {
                let slots = restored["other_slots"].as_array_mut().context("Deep scalar slots")?;
                for word in erc20_balances_tools::oft_roles::admin_words().iter().rev() {
                    ensure!(
                        slots.pop() == Some(json!(word)),
                        "only the three appended Deep fixed admin words may be removed"
                    );
                }
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
        let mut producer_versions = BTreeMap::<i32, u64>::new();
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
            if matches!(mode, Mode::PToken | Mode::Securities) {
                ensure!(
                    matches!(block.ver, 4 | 5),
                    "selected coupled replay requires reviewed Extended version4/5 at {height}"
                );
            }
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
            if mode == Mode::Oft {
                *selected_counts
                    .get_mut(erc20_balances_tools::oft_roles::DEEP)
                    .unwrap()
                    .entry("persisted_fixed_role_admin_writes")
                    .or_default() += deep_admin_writes(&block);
            }
            if mode == Mode::Tagger {
                *selected_counts
                    .get_mut(erc20_balances_tools::tagger_role::CONTRACT)
                    .unwrap()
                    .entry("persisted_outer_role_admin_writes")
                    .or_default() += tagger_admin_writes(&block)?;
            }
            let mut actual = erc20_balances::project(&block, &layouts).map_err(|e| anyhow::anyhow!("native map failed at {height}: {e}"))?;
            if matches!(mode, Mode::PToken | Mode::Securities) {
                // Each changing boolean belongs to exactly one complete operation
                // only after the coupled projector succeeds. This is not a claim
                // that a captured producer exposes operations absent from this window.
                for selected in selected_counts.values_mut() {
                    selected.insert("validated_coupled_role_operations", selected["persisted_role_membership_writes"]);
                }
            }
            if mode == Mode::Securities {
                for (contract, count) in securities_admin_writes(&block, &candidates)? {
                    *selected_counts
                        .get_mut(&contract)
                        .unwrap()
                        .entry("persisted_fixed_issuer_admin_writes")
                        .or_default() += count;
                }
            }
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
            *producer_versions.entry(block.ver).or_default() += 1;
            *counts.entry("empty_native_blocks").or_default() += u64::from(native_rows.is_empty());
            *counts.entry("canonical_same_block_rows").or_default() += same_block;
            *counts.entry("native_only_vs_canonical_reference_rows").or_default() += extra;
            *counts.entry("canonical_configured_reference_rows").or_default() += filtered_reference;
            *counts.entry("canonical_reference_only_retained_matches").or_default() += carried;
            *counts.entry("canonical_reference_only_unknown_cold_holders").or_default() += cold;
            // Preserve progress if a later mapper/identity/ledger check fails.
            // Never insert fake empty rows to bridge a refusal.
            report["counts"] = json!(counts);
            report["completed_block_producer_versions"] = json!(producer_versions);
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
                json!({"block":height,"producer_version":block.ver,"hash":hash,"parent_hash":parent,"path":path,"size":bytes.len(),"sha256":digest(&bytes),"native_protobuf_sha256":digest(&actual.encode_to_vec()),"historical_protobuf_sha256":digest(&historical[&height].encode_to_vec())})
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
            Some("--tagger") => Mode::Tagger,
            Some("--artx") => Mode::Artx,
            Some("--oft") => Mode::Oft,
            Some("--ptoken") => Mode::PToken,
            Some("--securities") => Mode::Securities,
            _ => bail!(
                "expected --burnmint, --point-bedrock, --fhe-b2, --bas, --tagger, --artx, --oft, --ptoken or --securities, followed by <original package root>"
            ),
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
        if mode == Mode::Tagger {
            report["scope"] = json!("One NOT-QUALIFIED Tagger exact outer-admin and membership candidate applied to unchanged 431; saved CBOR-only reconstruction and native parity do not qualify deployment, current owner or replacement package. Independent token/dependency source pins for the flattened capture remain unresolved. Canonical values never seed retained state.");
        }
        if mode == Mode::Artx {
            report["scope"] = json!("One NOT-QUALIFIED Artx proxy membership path applied to historical431; exact saved proxy/implementation/compiler/constructor bindings and native parity are not current pointer/owner, deployment or replacement-package qualification. Public token source gap remains unresolved. Zero initial supply and canonical values never seed retained state.");
        }
        if mode == Mode::Oft {
            report["scope"] = json!("Two NOT-QUALIFIED Kgen/Deep exact plain-role membership paths plus three fixed Deep admin words applied to unchanged historical431. Full saved capture/compiler/constructor/runtime reconstruction and native parity do not qualify deployed role calls, initializer history, current proxy state or replacement packages. Public source gaps and independent forwarder-array/long-bytes limitations remain; creation is refused. Canonical values never seed retained state.");
        }
        if mode == Mode::PToken {
            report["scope"] = json!("One NOT-QUALIFIED PToken coupled root5/root6 candidate applied to historical431; exact compiled operation proof and complete saved replay are not runtime/package, initial coherent state or real producer role-write qualification. The template is selected-build specific, requires Extended4/5 and positive actual root-call begin; v3 fallback is refused. Public token source gap remains. Canonical values never seed state.");
        }
        if mode == Mode::Securities {
            report["scope"] = json!("Seventeen NOT-QUALIFIED SecuritiesToken proxy candidates replace only their broad namespaced role permission with the exact selected solc0.8.24/OZ5.3 coupled template and one explicit zero-only ISSUER admin word. Historical431 remains unchanged. Extended4/5 with real frames is required. Saved parity is not initial coherence, initializer/client/proxy history, producer role visibility, runtime/package or live qualification. Eight primary source gaps and absent on-chain creation/deployment evidence remain. Canonical values never seed state.");
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
        fn securities_counters_separate_complete_operations_from_fixed_admin_writes() {
            use erc20_balances_tools::securities_role as bound;
            let candidates: Value = serde_json::from_str(include_str!("../../../tests/fixtures/securities-coupled-role-candidate/layouts.json")).unwrap();
            let cases: Value = serde_json::from_str(include_str!("../../../docs/evidence/securities-operation-proof-20260928-transcripts.json")).unwrap();
            let layouts = erc20_balances::layout::parse(&candidates.to_string()).unwrap();
            for profile in candidates.as_array().unwrap() {
                let contract = profile["contract"].as_str().unwrap();
                let address = decode_hex(&profile["contract"]).unwrap();
                for (name, expected) in [
                    ("grant_empty", 1),
                    ("grant_zero_empty", 1),
                    ("revoke_middle", 1),
                    ("revoke_zero_sole", 1),
                    ("grant_duplicate", 0),
                ] {
                    let c = cases.as_array().unwrap().iter().find(|c| c["name"] == name).unwrap();
                    let call = eth::Call {
                        address: address.clone(),
                        begin_ordinal: 1,
                        end_ordinal: 2000,
                        keccak_preimages: c["execution"]["keccaks"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|k| {
                                (
                                    k["output"].as_str().unwrap().trim_start_matches("0x").to_owned(),
                                    k["input"].as_str().unwrap().to_owned(),
                                )
                            })
                            .collect(),
                        storage_changes: c["execution"]["writes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|w| eth::StorageChange {
                                address: address.clone(),
                                key: decode_hex(&w["key"]).unwrap(),
                                old_value: decode_hex(&w["old"]).unwrap(),
                                new_value: decode_hex(&w["new"]).unwrap(),
                                ordinal: w["step"].as_u64().unwrap() + 10,
                            })
                            .collect(),
                        ..Default::default()
                    };
                    let mut b = eth::Block {
                        ver: 5,
                        number: 122288046,
                        hash: vec![7; 32],
                        header: Some(eth::BlockHeader {
                            number: 122288046,
                            parent_hash: vec![6; 32],
                            state_root: vec![8; 32],
                            ..Default::default()
                        }),
                        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
                        transaction_traces: vec![eth::TransactionTrace {
                            status: eth::TransactionTraceStatus::Succeeded as i32,
                            begin_ordinal: 1,
                            end_ordinal: 2000,
                            calls: vec![call],
                            ..Default::default()
                        }],
                        ..Default::default()
                    };
                    erc20_balances::project(&b, &layouts).unwrap();
                    assert_eq!(membership_writes(&b, &candidates).unwrap()[contract], expected);
                    assert_eq!(securities_admin_writes(&b, &candidates).unwrap()[contract], 0);
                    for mutation in ["missing", "failed", "reverted", "address", "noop", "wide_old", "wide_new", "short_key"] {
                        let mut changed = b.clone();
                        let tx = &mut changed.transaction_traces[0];
                        match mutation {
                            "missing" => tx.calls[0].keccak_preimages.clear(),
                            "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                            "reverted" => tx.calls[0].state_reverted = true,
                            _ => {
                                for w in &mut tx.calls[0].storage_changes {
                                    match mutation {
                                        "address" => w.address = vec![0; 20],
                                        "noop" => w.new_value = w.old_value.clone(),
                                        "wide_old" => w.old_value = vec![0; 33],
                                        "wide_new" => w.new_value = vec![1; 33],
                                        "short_key" => {
                                            w.key.pop();
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                            }
                        }
                        assert_eq!(membership_writes(&changed, &candidates).unwrap()[contract], 0, "{name}/{mutation}");
                    }
                    b.transaction_traces[0].calls[0].storage_changes.push(eth::StorageChange {
                        address: address.clone(),
                        key: hex::decode(&bound::ADMIN_SLOT[2..]).unwrap(),
                        old_value: vec![1],
                        new_value: vec![0],
                        ordinal: 1500,
                    });
                    erc20_balances::project(&b, &layouts).unwrap();
                    assert_eq!(membership_writes(&b, &candidates).unwrap()[contract], expected);
                    assert_eq!(securities_admin_writes(&b, &candidates).unwrap()[contract], 1);
                    for mutation in ["failed", "reverted", "address", "key", "noop", "nonzero", "wide_old", "wide_new"] {
                        let mut changed = b.clone();
                        let tx = &mut changed.transaction_traces[0];
                        match mutation {
                            "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                            "reverted" => tx.calls[0].state_reverted = true,
                            _ => {
                                let w = tx.calls[0].storage_changes.last_mut().unwrap();
                                match mutation {
                                    "address" => w.address = vec![0; 20],
                                    "key" => w.key[0] ^= 1,
                                    "noop" => w.old_value = vec![0],
                                    "nonzero" => w.new_value = vec![1],
                                    "wide_old" => w.old_value = vec![1; 33],
                                    "wide_new" => w.new_value = vec![0; 33],
                                    _ => unreachable!(),
                                }
                            }
                        }
                        assert_eq!(securities_admin_writes(&changed, &candidates).unwrap()[contract], 0, "{mutation}");
                    }
                }
            }
        }

        #[test]
        fn ptoken_membership_counter_uses_frozen_operations_and_exact_persisted_witnesses() {
            use erc20_balances_tools::ptoken_role as bound;
            let candidates: Value = serde_json::from_str(include_str!("../../../tests/fixtures/ptoken-coupled-role-candidate/layouts.json")).unwrap();
            let cases: Value = serde_json::from_str(include_str!("../../../docs/evidence/ptoken-operation-proof-20260928-transcripts.json")).unwrap();
            for (name, expected) in [
                ("grant_empty", 1),
                ("grant_zero_empty", 1),
                ("remove_middle", 1),
                ("remove_only_zero", 1),
                ("grant_duplicate", 0),
            ] {
                let c = cases.as_array().unwrap().iter().find(|c| c["name"] == name).unwrap();
                let call = eth::Call {
                    begin_ordinal: 1,
                    end_ordinal: 2000,
                    keccak_preimages: c["execution"]["keccaks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|k| {
                            (
                                k["output"].as_str().unwrap().trim_start_matches("0x").to_owned(),
                                k["input"].as_str().unwrap().to_owned(),
                            )
                        })
                        .collect(),
                    storage_changes: c["execution"]["writes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|w| eth::StorageChange {
                            address: hex::decode(&bound::CONTRACT[2..]).unwrap(),
                            key: decode_hex(&w["key"]).unwrap(),
                            old_value: decode_hex(&w["old"]).unwrap(),
                            new_value: decode_hex(&w["new"]).unwrap(),
                            ordinal: w["step"].as_u64().unwrap() + 10,
                        })
                        .collect(),
                    ..Default::default()
                };
                let block = eth::Block {
                    ver: 5,
                    number: 122288046,
                    hash: vec![7; 32],
                    header: Some(eth::BlockHeader {
                        number: 122288046,
                        parent_hash: vec![6; 32],
                        state_root: vec![8; 32],
                        ..Default::default()
                    }),
                    detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
                    transaction_traces: vec![eth::TransactionTrace {
                        status: eth::TransactionTraceStatus::Succeeded as i32,
                        begin_ordinal: 1,
                        end_ordinal: 2000,
                        calls: vec![call],
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                let layouts = erc20_balances::layout::parse(&candidates.to_string()).unwrap();
                erc20_balances::project(&block, &layouts).unwrap();
                assert_eq!(membership_writes(&block, &candidates).unwrap()[bound::CONTRACT], expected, "{name}");
                for field in ["missing", "failed", "reverted", "address", "noop", "wide_old", "wide_new", "short_key"] {
                    let mut b = block.clone();
                    let tx = &mut b.transaction_traces[0];
                    match field {
                        "missing" => tx.calls[0].keccak_preimages.clear(),
                        "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                        "reverted" => tx.calls[0].state_reverted = true,
                        _ => {
                            for w in &mut tx.calls[0].storage_changes {
                                match field {
                                    "address" => w.address[0] ^= 1,
                                    "noop" => w.new_value = w.old_value.clone(),
                                    "wide_old" => w.old_value = vec![0; 33],
                                    "wide_new" => w.new_value = vec![1; 33],
                                    _ => {
                                        w.key.remove(0);
                                    }
                                }
                            }
                        }
                    }
                    assert_eq!(membership_writes(&b, &candidates).unwrap()[bound::CONTRACT], 0, "{name}/{field}");
                }
                let mut bad = block;
                bad.transaction_traces[0].calls[0]
                    .keccak_preimages
                    .insert(hex::encode([0; 32]), hex::encode([1; 64]));
                assert!(membership_writes(&bad, &candidates).is_err());
            }
        }

        #[test]
        fn deep_admin_count_requires_three_fixed_locations_and_persisted_well_formed_change() {
            use erc20_balances_tools::oft_roles as bound;
            let call = eth::Call {
                storage_changes: bound::admin_words()
                    .iter()
                    .enumerate()
                    .map(|(i, key)| eth::StorageChange {
                        address: hex::decode(&bound::DEEP[2..]).unwrap(),
                        key: hex::decode(&key[2..]).unwrap(),
                        old_value: vec![0],
                        new_value: vec![1],
                        ordinal: i as u64 + 1,
                    })
                    .collect(),
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
            assert_eq!(deep_admin_writes(&block), 6);
            block.system_calls[0].state_reverted = true;
            assert_eq!(deep_admin_writes(&block), 3);
            for field in ["key", "address", "kgen", "noop", "failed", "reverted", "long_old", "long_new"] {
                let mut b = block.clone();
                let tx = &mut b.transaction_traces[0];
                for w in &mut tx.calls[0].storage_changes {
                    match field {
                        "key" => w.key[0] ^= 1,
                        "address" => w.address[0] ^= 1,
                        "kgen" => w.address = hex::decode(&bound::KGEN[2..]).unwrap(),
                        "noop" => w.old_value = vec![0, 1],
                        "long_old" => w.old_value = vec![0; 33],
                        "long_new" => w.new_value = vec![0; 33],
                        _ => (),
                    }
                }
                if field == "failed" {
                    tx.status = eth::TransactionTraceStatus::Failed as i32;
                }
                if field == "reverted" {
                    tx.calls[0].state_reverted = true;
                }
                assert_eq!(deep_admin_writes(&b), 0, "{field}");
            }
        }

        #[test]
        fn tagger_arbitrary_admin_count_requires_exact_preimage_offset_and_persistence() {
            use erc20_balances_tools::tagger_role as bound;
            let address = hex::decode(&bound::CONTRACT[2..]).unwrap();
            let root = hex::decode(&bound::root(6)[2..]).unwrap();
            let candidates: Value = serde_json::from_str(include_str!("../../../tests/fixtures/tagger-role-candidate/layouts.json")).unwrap();
            for role in [
                [0; 32],
                erc20_balances::hash(b"ROLE_DEPLOYER"),
                erc20_balances::hash(b"ROLE_OPERATOR"),
                [0xff; 32],
            ] {
                let image = [role.as_slice(), root.as_slice()].concat();
                let outer = erc20_balances::hash(&image);
                let admin = primitive_types::U256::from_big_endian(&outer).overflowing_add(primitive_types::U256::one()).0;
                let mut admin_key = [0; 32];
                admin.to_big_endian(&mut admin_key);
                let call = eth::Call {
                    keccak_preimages: [(hex::encode(outer), hex::encode(&image))].into_iter().collect(),
                    storage_changes: vec![eth::StorageChange {
                        address: address.clone(),
                        key: admin_key.to_vec(),
                        old_value: vec![0],
                        new_value: vec![0xff; 32],
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
                assert_eq!(tagger_admin_writes(&block).unwrap(), 2);
                assert_eq!(membership_writes(&block, &candidates).unwrap()[bound::CONTRACT], 0);
                block.system_calls.clear();
                assert_eq!(tagger_admin_writes(&block).unwrap(), 1);
                for field in [
                    "outer",
                    "plus2",
                    "address",
                    "noop",
                    "failed",
                    "reverted",
                    "missing",
                    "wide_old",
                    "wide_new",
                    "wrong_root",
                ] {
                    let mut b = block.clone();
                    let tx = &mut b.transaction_traces[0];
                    match field {
                        "outer" => tx.calls[0].storage_changes[0].key = outer.to_vec(),
                        "plus2" => {
                            let mut key = [0; 32];
                            admin.overflowing_add(primitive_types::U256::one()).0.to_big_endian(&mut key);
                            tx.calls[0].storage_changes[0].key = key.to_vec();
                        }
                        "address" => tx.calls[0].storage_changes[0].address[0] ^= 1,
                        "noop" => tx.calls[0].storage_changes[0].old_value = vec![0xff; 32],
                        "failed" => tx.status = eth::TransactionTraceStatus::Failed as i32,
                        "reverted" => tx.calls[0].state_reverted = true,
                        "missing" => tx.calls[0].keccak_preimages.clear(),
                        "wide_old" => tx.calls[0].storage_changes[0].old_value = vec![0; 33],
                        "wide_new" => tx.calls[0].storage_changes[0].new_value = vec![1; 33],
                        _ => {
                            let mut changed = image.clone();
                            changed[63] = 7;
                            tx.calls[0].keccak_preimages = [(hex::encode(erc20_balances::hash(&changed)), hex::encode(changed))].into_iter().collect();
                        }
                    }
                    assert_eq!(tagger_admin_writes(&b).unwrap(), 0, "{field}");
                }
                let mut bad = block.clone();
                bad.transaction_traces[0].calls[0]
                    .keccak_preimages
                    .insert(hex::encode(outer), hex::encode([0; 64]));
                assert!(tagger_admin_writes(&bad).is_err());
                // A real membership chain must be counted only as membership.
                let mut member = [0; 32];
                member[12..].fill(0x33);
                let inner = [member.as_slice(), outer.as_slice()].concat();
                let key = erc20_balances::hash(&inner);
                block.transaction_traces[0].calls[0]
                    .keccak_preimages
                    .insert(hex::encode(key), hex::encode(inner));
                block.transaction_traces[0].calls[0].storage_changes[0].key = key.to_vec();
                block.transaction_traces[0].calls[0].storage_changes[0].new_value = vec![1];
                assert_eq!(tagger_admin_writes(&block).unwrap(), 0);
                assert_eq!(membership_writes(&block, &candidates).unwrap()[bound::CONTRACT], 1);
                block.transaction_traces[0].calls[0].storage_changes[0].old_value = vec![0, 1];
                assert_eq!(membership_writes(&block, &candidates).unwrap()[bound::CONTRACT], 0);
            }
        }

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
