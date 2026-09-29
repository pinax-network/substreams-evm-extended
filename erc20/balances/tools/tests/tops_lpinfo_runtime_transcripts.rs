#![cfg(not(target_arch = "wasm32"))]
//! Bound original-runtime effects adapted to synthetic Extended records. VM
//! steps are synthetic ordinals, not a claim about a real producer. In particular
//! the two scripted read-only external responses are not fabricated call frames.
//! Historical cleanup-harness effects are never used as deployed-runtime order.
use anyhow::{ensure, Context, Result};
use erc20_balances::{hash, layout};
use erc20_balances_tools::{tops_lpinfo as candidate, tops_proof};
use primitive_types::U256;
use prost::Message;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::OnceLock};
use substreams_ethereum::pb::eth::v2 as eth;

const CLEANUP: &[u8] = include_bytes!("../../docs/evidence/tops-runtime-cleanup-proof-20260929-transcripts.json");
const APPEND: &[u8] = include_bytes!("../../docs/evidence/tops-operation-proof-20260929-transcripts.json");
const BASELINE: &[u8] = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
const HISTORICAL: &[u8] = include_bytes!("../../tests/fixtures/bsc-exclusions-20260928/tops-123561227-tx63.pb");
const RUNTIME_SHA: &str = "8c5dbe78abde6dfb7b6a48e18fdbe55272b76217fca6d8120fe0d24221998d4f";
const OFFSET: u64 = 100;
const COPY: [u64; 3] = [11480, 11488, 11497];
const POP: [u64; 4] = [11321, 11327, 11329, 11331];
const APPEND_PCS: [u64; 4] = [5355, 5371, 5377, 5384];

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap().trim_start_matches("0x")).unwrap()
}
fn word(v: &Value) -> U256 {
    let b = bytes(v);
    assert_eq!(b.len(), 32);
    U256::from_big_endian(&b)
}
fn number(n: u64) -> Vec<u8> {
    let mut b = vec![0; 32];
    b[24..].copy_from_slice(&n.to_be_bytes());
    b
}
fn account() -> Vec<u8> {
    hex::decode(candidate::CONTRACT.trim_start_matches("0x")).unwrap()
}
fn address(v: &Value) -> Vec<u8> {
    let b = bytes(v);
    assert_eq!(b.len(), 32);
    assert_eq!(&b[..12], &[0; 12]);
    b[12..].to_vec()
}
fn runtime() -> &'static Vec<u8> {
    static CODE: OnceLock<Vec<u8>> = OnceLock::new();
    CODE.get_or_init(|| {
        let capture = tops_proof::verify_capture(include_bytes!("../../tests/fixtures/tops-operation-proof/capture.json")).unwrap();
        let code = bytes(&capture["runtimeBytecode"]["onchainBytecode"]);
        assert_eq!(code.len(), 22_323);
        assert_eq!(sha(&code), RUNTIME_SHA);
        assert_eq!(format!("0x{}", hex::encode(hash(&code))), tops_proof::RUNTIME);
        code
    })
}
fn cleanup_cases() -> &'static Vec<Value> {
    static CASES: OnceLock<Vec<Value>> = OnceLock::new();
    CASES.get_or_init(|| {
        assert_eq!(sha(CLEANUP), "6ea61762e277c6f0b345bdf8b5fe6c86ec13d15d91b3ba73d3de04882686be02");
        let cases: Vec<Value> = serde_json::from_slice(CLEANUP).unwrap();
        assert_eq!(cases.len(), 138);
        for v in &cases {
            assert_eq!(v["code_sha256"], RUNTIME_SHA);
        }
        cases
    })
}
fn append_cases() -> &'static Vec<Value> {
    static CASES: OnceLock<Vec<Value>> = OnceLock::new();
    CASES.get_or_init(|| {
        assert_eq!(sha(APPEND), "f4f792d1ee0ad68e8d510e11ab2166d13d1f2d495ae32d5e2a817d1086e2011f");
        serde_json::from_slice(APPEND).unwrap()
    })
}
fn named(name: &str) -> &'static Value {
    let matches: Vec<_> = cleanup_cases().iter().filter(|v| v["name"] == name).collect();
    assert_eq!(matches.len(), 1, "unique compact case {name}");
    matches[0]
}
fn layouts() -> Vec<layout::VerifiedLayout> {
    layout::parse(&candidate::candidate(BASELINE).unwrap().to_string()).unwrap()
}
fn effects<'a>(v: &'a Value, kind: &str) -> &'a [Value] {
    v["execution"][kind].as_array().unwrap()
}
fn pc(v: &Value) -> u64 {
    v["pc"].as_u64().unwrap()
}
fn time(v: &Value) -> U256 {
    word(if v["context"].is_object() {
        &v["context"]["timestamp"]
    } else {
        &v["timestamp"]
    })
}
fn check_effect(v: &Value, opcode: u8) {
    let p = pc(v) as usize;
    assert_eq!(runtime()[p], opcode);
    assert_eq!(v["source"]["pc"].as_u64().unwrap(), p as u64);
    assert_eq!(v["source"]["path"], tops_proof::SOURCE);
    assert_eq!(v["source"]["source_sha256"], tops_proof::SOURCE_SHA);
}

// This adapter refuses unrepresentable timestamps and host failures. Reverted
// source attempts retain their exact effects under a reverted synthetic frame;
// the production persistence rules must filter them. VM prestate is deliberately
// NOT injected as invented storage observations or equality anchors.
fn synthetic(v: &Value, version: i32) -> Result<eth::Block> {
    ensure!(v["code_sha256"] == RUNTIME_SHA, "not the pinned original runtime");
    if !v["variant"].is_null() {
        ensure!(v["variant"] == "captured_full_runtime", "not original full-runtime source kind");
    }
    let exit = v["execution"]["exit"]["kind"].as_str().context("exit kind")?;
    ensure!(matches!(exit, "return" | "revert"), "host failure is not a chain outcome");
    let now = time(v);
    ensure!(now <= U256::from(i64::MAX as u64), "full U256 timestamp does not fit Extended seconds");
    let mut c = eth::Call {
        index: 1,
        address: account(),
        caller: address(&v["caller"]),
        input: bytes(&v["calldata"]),
        begin_ordinal: 1,
        state_reverted: exit == "revert",
        ..Default::default()
    };
    let mut last_step = 0;
    for e in effects(v, "writes") {
        check_effect(e, 0x55);
        let step = e["step"].as_u64().unwrap();
        ensure!(step > last_step, "source SSTORE order");
        last_step = step;
        c.storage_changes.push(eth::StorageChange {
            address: account(),
            key: bytes(&e["key"]),
            old_value: bytes(&e["old"]),
            new_value: bytes(&e["new"]),
            ordinal: step + OFFSET,
        });
    }
    for e in effects(v, "keccaks") {
        check_effect(e, 0x20);
        let input = bytes(&e["input"]);
        ensure!(hash(&input).as_slice() == bytes(&e["output"]), "complete verified Keccak preimage");
        c.keccak_preimages.insert(hex::encode(bytes(&e["output"])), hex::encode(input));
    }
    for (index, e) in effects(v, "logs").iter().enumerate() {
        let topics = e["topics"].as_array().unwrap();
        check_effect(e, 0xa0 + topics.len() as u8);
        c.logs.push(eth::Log {
            address: account(),
            topics: topics.iter().map(bytes).collect(),
            data: bytes(&e["data"]),
            index: index as u32,
            ordinal: e["step"].as_u64().unwrap() + OFFSET,
            ..Default::default()
        });
    }
    // Preserve the spacing around observed external calls. Their responses were
    // scripted VM inputs; no child frame is invented to certify their behavior.
    let mut final_step = last_step;
    for kind in ["reads", "keccaks", "logs"] {
        for e in effects(v, kind) {
            final_step = final_step.max(e["step"].as_u64().unwrap());
        }
    }
    if let Some(calls) = v["context_witness"]["calls"].as_array() {
        for e in calls {
            final_step = final_step.max(e["step"].as_u64().unwrap());
        }
    }
    c.end_ordinal = final_step + 2 * OFFSET;
    let origin = if v["context"].is_object() {
        address(&v["context"]["origin"])
    } else {
        // The old append proof never executed ORIGIN. This frame envelope is a
        // synthetic choice, not a claim that the proof supplied transaction origin.
        c.caller.clone()
    };
    let mut header = eth::BlockHeader {
        number: 100,
        parent_hash: vec![2; 32],
        state_root: vec![3; 32],
        timestamp: Some(Default::default()),
        ..Default::default()
    };
    header.timestamp.as_mut().unwrap().seconds = now.low_u64() as i64;
    Ok(eth::Block {
        ver: version,
        number: 100,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(header),
        transaction_traces: vec![eth::TransactionTrace {
            from: origin,
            to: account(),
            input: c.input.clone(),
            status: if exit == "return" { 1 } else { 3 },
            begin_ordinal: c.begin_ordinal,
            end_ordinal: c.end_ordinal,
            calls: vec![c],
            ..Default::default()
        }],
        ..Default::default()
    })
}
fn call(b: &mut eth::Block) -> &mut eth::Call {
    &mut b.transaction_traces[0].calls[0]
}

// Independent output oracle: captured final storage for the exact root5 leaves
// touched by changed original-runtime SSTOREs. Metadata keys never become balances.
fn expected_balances(v: &Value) -> BTreeMap<Vec<u8>, String> {
    if v["execution"]["exit"]["kind"] != "return" {
        return BTreeMap::new();
    }
    let leaves: BTreeMap<_, _> = effects(v, "keccaks")
        .iter()
        .filter_map(|e| {
            let input = bytes(&e["input"]);
            (input.len() == 64 && input[..12] == [0; 12] && input[32..] == number(5)).then(|| (bytes(&e["output"]), input[12..32].to_vec()))
        })
        .collect();
    let committed: BTreeMap<_, _> = effects(v, "committed_storage").iter().map(|e| (bytes(&e["key"]), word(&e["value"]))).collect();
    let mut expected = BTreeMap::new();
    for e in effects(v, "writes").iter().filter(|e| e["old"] != e["new"]) {
        let key = bytes(&e["key"]);
        if let Some(owner) = leaves.get(&key).filter(|owner| owner.iter().any(|b| *b != 0)) {
            expected.insert(owner.clone(), committed[&key].to_string());
        }
    }
    expected
}
fn assert_output(v: &Value, b: &eth::Block) -> candidate::Counts {
    let (events, counts) = candidate::project_counted(b, &layouts()).unwrap_or_else(|e| panic!("{}: {e:#}", v["name"]));
    for row in &events.balances {
        assert_eq!(row.contract.as_ref().unwrap(), &account());
    }
    let actual: BTreeMap<_, _> = events.balances.iter().map(|e| (e.address.clone(), e.amount.clone())).collect();
    assert_eq!(events.balances.len(), actual.len(), "no duplicated output holders");
    assert_eq!(actual, expected_balances(v), "{}", v["name"]);
    counts
}

#[test]
fn source_identity_excludes_harness_and_preserves_complete_runtime_context() {
    runtime();
    assert_eq!(append_cases().len(), 49);
    let harness = append_cases().iter().find(|v| v["variant"] == "source_cleanup_harness").unwrap();
    assert!(synthetic(harness, 5).is_err());
    let v = named("prefix_n2_k1_mode1");
    let b = synthetic(v, 5).unwrap();
    let c = &b.transaction_traces[0].calls[0];
    assert_eq!(b.transaction_traces[0].calls.len(), 1);
    assert_eq!(c.input, bytes(&v["calldata"]));
    assert_eq!(c.caller, address(&v["caller"]));
    assert_eq!(b.transaction_traces[0].from, address(&v["context"]["origin"]));
    assert_ne!(b.transaction_traces[0].from, c.caller);
    assert_eq!(v["context_witness"]["consumed_calls"], 2);
    assert_eq!(v["context_witness"]["consumed_gas"], 2);
    for (i, p) in [7758, 8143].into_iter().enumerate() {
        let external = &v["context_witness"]["calls"][i];
        assert_eq!(pc(external), p);
        assert_eq!(external["result"]["kind"], "return");
        let ordinal = external["step"].as_u64().unwrap() + OFFSET;
        assert!(c.begin_ordinal < ordinal && ordinal < c.storage_changes[0].ordinal);
    }
    for (row, effect) in c.storage_changes.iter().zip(effects(v, "writes")) {
        assert_eq!(
            (&row.key, &row.old_value, &row.new_value),
            (&bytes(&effect["key"]), &bytes(&effect["old"]), &bytes(&effect["new"]))
        );
        assert_eq!(row.ordinal, effect["step"].as_u64().unwrap() + OFFSET);
    }
}

#[test]
fn all_bounded_original_runtime_prefixes_preserve_balances_and_exact_pc_order() {
    for version in [4, 5] {
        let mut tested = 0;
        for length in 0..=6 {
            for removed in 0..=length {
                for mode in 0..=2 {
                    let v = named(&format!("prefix_n{length}_k{removed}_mode{mode}"));
                    let b = synthetic(v, version).unwrap();
                    let counts = assert_output(v, &b);
                    let cleanup = removed > 0 && mode != 0;
                    let mut expected_pcs = Vec::new();
                    if cleanup {
                        for _ in 0..length - removed {
                            expected_pcs.extend(COPY);
                        }
                        for _ in 0..removed {
                            expected_pcs.extend(POP);
                        }
                        expected_pcs.push(11397);
                    }
                    expected_pcs.extend([8501, 19870]);
                    assert_eq!(effects(v, "writes").iter().map(pc).collect::<Vec<_>>(), expected_pcs, "{}", v["name"]);
                    assert_eq!(counts.validated_appends, 0);
                    assert_eq!(counts.validated_cleanups, u64::from(cleanup));
                    assert_eq!(counts.observed_length_decrements, if cleanup { removed as u64 } else { 0 });
                    assert_eq!(counts.observed_metadata_stores, (expected_pcs.len() - 2) as u64);
                    tested += 1;
                }
            }
        }
        assert_eq!(tested, 84);
    }
}

#[test]
fn other_successful_runtime_contexts_preserve_final_balances_and_allowances() {
    let mut tested = 0;
    for v in cleanup_cases().iter().filter(|v| {
        v["execution"]["exit"]["kind"] == "return"
            && matches!(
                v["signature"].as_str(),
                Some("transfer(address,uint256)" | "transferFrom(address,address,uint256)")
            )
            && !v["name"].as_str().unwrap().starts_with("prefix_")
            && v["name"] != "maximum_timestamp"
    }) {
        for version in [4, 5] {
            assert_output(v, &synthetic(v, version).unwrap());
        }
        tested += 1;
    }
    assert_eq!(tested, 21);
}

#[test]
fn original_runtime_appends_keep_equal_stores_and_refuse_unbounded_lengths() {
    let mut projected = 0;
    let mut refused_timestamp = 0;
    for v in append_cases()
        .iter()
        .filter(|v| v["signature"] == "createLPInfo(address,uint256)" && v["execution"]["exit"]["kind"] == "return")
    {
        assert_eq!(v["variant"], "captured_full_runtime");
        assert_eq!(effects(v, "writes").iter().map(pc).collect::<Vec<_>>(), APPEND_PCS);
        if time(v) > U256::from(i64::MAX as u64) {
            assert!(synthetic(v, 5).is_err());
            refused_timestamp += 1;
            continue;
        }
        let b = synthetic(v, 5).unwrap();
        if v["name"] == "append_length_18446744073709551615" {
            assert!(
                candidate::project_counted(&b, &layouts()).is_err(),
                "source success is outside six-record permission"
            );
            continue;
        }
        for version in [4, 5] {
            let counts = assert_output(v, &synthetic(v, version).unwrap());
            assert_eq!(counts.validated_appends, 1);
            assert_eq!(counts.validated_cleanups, 0);
            assert_eq!(counts.observed_metadata_stores, 4);
            assert_eq!(counts.observed_equal_metadata_stores, u64::from(time(v).is_zero()));
        }
        projected += 1;
    }
    assert_eq!((projected, refused_timestamp), (3, 1));
    assert!(synthetic(named("maximum_timestamp"), 5).is_err());
    let mut reverted_lengths = 0;
    for v in append_cases()
        .iter()
        .filter(|v| v["name"].as_str().unwrap().starts_with("append_length_") && v["execution"]["exit"]["kind"] == "revert")
    {
        assert!(effects(v, "writes").is_empty());
        assert_eq!(assert_output(v, &synthetic(v, 5).unwrap()), candidate::Counts::default());
        reverted_lengths += 1;
    }
    assert_eq!(reverted_lengths, 2);
}

#[test]
fn omitted_equalities_need_observed_grounding_not_the_vm_prestate() {
    let grounded = named("prefix_n3_k1_mode1");
    let mut b = synthetic(grounded, 5).unwrap();
    assert_eq!(call(&mut b).storage_changes.iter().filter(|r| r.old_value == r.new_value).count(), 1);
    call(&mut b).storage_changes.retain(|r| r.old_value != r.new_value);
    // The omitted expiry copy's old value was the earlier copy's new value;
    // its new value is independently witnessed by the final tail clear.
    assert_output(grounded, &b);
    for name in ["at_expiry", "wrapped_expired_nonzero"] {
        let v = named(name);
        let mut b = synthetic(v, 5).unwrap();
        assert_output(v, &b);
        call(&mut b).storage_changes.retain(|r| r.old_value != r.new_value);
        assert!(
            candidate::project_counted(&b, &layouts()).is_err(),
            "{name}: omitted zero creation time is unknown to the mapper"
        );
    }
    for v in append_cases()
        .iter()
        .filter(|v| v["signature"] == "createLPInfo(address,uint256)" && v["execution"]["exit"]["kind"] == "return" && time(v).is_zero())
    {
        let mut b = synthetic(v, 5).unwrap();
        call(&mut b).storage_changes.retain(|r| r.old_value != r.new_value);
        assert!(
            candidate::project_counted(&b, &layouts()).is_err(),
            "old zero of omitted createdAt is not established by timestamp zero"
        );
    }
}

#[test]
fn zero_and_wrapped_zero_prefixes_supply_no_cleanup_permission() {
    for name in ["wrapped_expired_zero", "all_zero_no_cleanup", "prefix_n6_k6_mode0"] {
        let v = named(name);
        assert_eq!(effects(v, "writes").iter().map(pc).collect::<Vec<_>>(), [8501, 19870]);
        let b = synthetic(v, 5).unwrap();
        let counts = assert_output(v, &b);
        assert_eq!(counts, candidate::Counts::default());
        // Add an independently written credit despite the source's zero-sum
        // gate. Complete known preimage alone cannot authorize that mutation.
        let owner = bytes(&v["context"]["origin"]);
        let image = [owner, number(32)].concat();
        let key = hash(&image);
        let mut mutated = b.clone();
        let c = call(&mut mutated);
        c.keccak_preimages.insert(hex::encode(key), hex::encode(image));
        c.storage_changes.push(eth::StorageChange {
            address: account(),
            key: key.to_vec(),
            old_value: number(0),
            new_value: number(1),
            ordinal: c.end_ordinal - 1,
        });
        assert!(candidate::project_counted(&mutated, &layouts()).is_err());
    }
}

#[test]
fn reverted_attempts_are_filtered_and_host_failures_are_never_chain_successes() {
    let mut reverts = 0;
    let mut unsupported = 0;
    for v in cleanup_cases() {
        match v["execution"]["exit"]["kind"].as_str().unwrap() {
            "revert" => {
                assert_eq!(v["execution"]["committed_storage"], v["prestate"]);
                assert_eq!(v["execution"]["committed_logs"], 0);
                let b = synthetic(v, 5).unwrap();
                assert_eq!(b.transaction_traces[0].calls[0].storage_changes.len(), effects(v, "writes").len());
                assert_eq!(assert_output(v, &b), candidate::Counts::default());
                reverts += 1;
            }
            "harness_failure" => {
                assert!(synthetic(v, 5).is_err());
                unsupported += 1;
            }
            "return" => {}
            other => panic!("unexpected outcome {other}"),
        }
    }
    assert_eq!((reverts, unsupported), (21, 9));
    assert_eq!(effects(named("late_allowance_6_7"), "writes").len(), 13);
}

#[test]
fn runtime_effect_order_context_and_changed_stages_are_required() {
    let v = named("prefix_n2_k1_mode1");
    let original = synthetic(v, 5).unwrap();
    assert_output(v, &original);
    for removed in 0..8 {
        let mut b = original.clone();
        call(&mut b).storage_changes.remove(removed);
        assert!(
            candidate::project_counted(&b, &layouts()).is_err(),
            "missing original runtime metadata stage {removed}"
        );
    }
    for mode in 0..4 {
        let mut b = original.clone();
        match mode {
            0 => {
                let rows = &mut call(&mut b).storage_changes;
                let a = rows[0].ordinal;
                rows[0].ordinal = rows[1].ordinal;
                rows[1].ordinal = a;
            }
            1 => b.transaction_traces[0].from = b.transaction_traces[0].calls[0].caller.clone(),
            2 => b.header.as_mut().unwrap().timestamp.as_mut().unwrap().seconds = 99,
            _ => call(&mut b).input[0] ^= 1,
        }
        assert!(
            candidate::project_counted(&b, &layouts()).is_err(),
            "changed original runtime context/order {mode}"
        );
    }
}

#[test]
fn original_23_store_capture_matches_words_order_and_uses_its_own_clock() {
    assert_eq!(sha(HISTORICAL), "29a063ed467467ffe4f1eba438cc422cbee89492679646656979db3eb1873e2c");
    let b = eth::Block::decode(HISTORICAL).unwrap();
    assert_eq!(b.number, 123_561_227);
    let tx = b.transaction_traces.iter().find(|tx| tx.index == 63).unwrap();
    let c = tx.calls.iter().find(|c| c.index == 14).unwrap();
    let v = named("saved_five_pops_all_23_original_runtime_stores");
    assert_eq!(c.input, bytes(&v["calldata"]));
    assert_eq!(c.caller, address(&v["caller"]));
    let mut stores: Vec<_> = c.storage_changes.iter().filter(|r| r.address == account()).collect();
    stores.sort_by_key(|r| r.ordinal);
    assert_eq!(stores.len(), 23);
    for (captured, effect) in stores.iter().zip(effects(v, "writes")) {
        assert_eq!(
            (&captured.key, &captured.old_value, &captured.new_value),
            (&bytes(&effect["key"]), &bytes(&effect["old"]), &bytes(&effect["new"]))
        );
        check_effect(effect, 0x55);
    }
    let expected_pcs: Vec<_> = POP.repeat(5).into_iter().chain([11397, 8501, 19870]).collect();
    assert_eq!(effects(v, "writes").iter().map(pc).collect::<Vec<_>>(), expected_pcs);
    let original_header = b.header.clone();
    let original_origin = tx.from.clone();
    let (events, counts) = candidate::project_counted(&b, &layouts()).unwrap();
    assert_eq!(events.balances.len(), 2);
    assert_output(v, &b);
    assert_eq!(
        (counts.validated_cleanups, counts.observed_length_decrements, counts.observed_metadata_stores),
        (1, 5, 21)
    );
    assert_eq!(b.header, original_header);
    assert_eq!(b.transaction_traces.iter().find(|tx| tx.index == 63).unwrap().from, original_origin);
    let historical_layouts: Vec<Value> = serde_json::from_slice(BASELINE).unwrap();
    let old = historical_layouts.into_iter().find(|v| v["contract"] == candidate::CONTRACT).unwrap();
    assert!(erc20_balances::project(&b, &layout::parse(&serde_json::json!([old]).to_string()).unwrap()).is_err());
    // The proof expressly supplied synthetic origin/time and external answers;
    // matching its 23 words does not make it a full historical transaction replay.
    assert!(v["historical_reference"]["scope"].as_str().unwrap().contains("synthetic"));
}
