#![cfg(not(target_arch = "wasm32"))]
//! Source-bound issue-61 candidates, never a replacement for qualified profiles.
use erc20_balances::{hash, layout, project};
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use substreams_ethereum::pb::eth::v2 as eth;

const CANDIDATE: &str = include_str!("fixtures/bsc-exclusions-20260928/take-candidate-NOT-QUALIFIED.json");
const BASELINE: &str = include_str!("fixtures/bsc-refined450-layouts.json");
const GUARD: &str = "9b779b17422d0df92223018b32b4d1fa46e071723d6817e2486d003becc55f00";
fn bytes(s: &str) -> Vec<u8> {
    hex::decode(s.trim_start_matches("0x")).unwrap()
}
fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/bsc-exclusions-20260928/cases.json")).unwrap()
}
fn case(name: &str) -> Value {
    cases().into_iter().find(|c| c["token"] == name).unwrap()
}
fn block(case: &Value) -> eth::Block {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bsc-exclusions-20260928")
        .join(case["fixture"].as_str().unwrap());
    let raw = std::fs::read(path).unwrap();
    assert_eq!(hex::encode(Sha256::digest(&raw)), case["fixture_sha256"]);
    let b = eth::Block::decode(raw.as_slice()).unwrap();
    assert_eq!(b.number, case["block"].as_u64().unwrap());
    assert_eq!(b.hash, bytes(case["hash"].as_str().unwrap()));
    assert_eq!(b.transaction_traces.len(), 1);
    assert_eq!(b.transaction_traces[0].hash, bytes(case["transaction"].as_str().unwrap()));
    b
}
fn take() -> (Value, eth::Block, Vec<layout::VerifiedLayout>) {
    let c = case("take");
    let b = block(&c);
    (c, b, layout::parse(CANDIDATE).unwrap())
}
fn mint_call(b: &mut eth::Block) -> &mut eth::Call {
    b.transaction_traces[0].calls.iter_mut().find(|c| c.index == 30).unwrap()
}
fn source(text: &str) -> Value {
    let v: Value = serde_json::from_str(text).unwrap();
    assert_eq!(v["qualified"], false);
    assert_eq!(v["chain_id"], 56);
    for (name, s) in v["sources"].as_object().unwrap() {
        assert_eq!(
            format!("0x{}", hex::encode(hash(s["content"].as_str().unwrap().as_bytes()))),
            v["source_hashes"][name]["keccak256"]
        );
    }
    let r = &v["runtime"];
    let mut compiled = bytes(r["recompiledBytecode"].as_str().unwrap());
    for t in r["transformations"].as_array().unwrap() {
        assert_eq!(t["reason"], "immutable");
        assert_eq!(t["type"], "replace");
        let id = t["id"].as_str().unwrap();
        let offset = t["offset"].as_u64().unwrap() as usize;
        let replacement = bytes(r["transformationValues"]["immutables"][id].as_str().unwrap());
        assert!(r["immutableReferences"][id]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["start"] == offset && p["length"] == replacement.len()));
        compiled[offset..offset + replacement.len()].copy_from_slice(&replacement);
    }
    assert_eq!(compiled, bytes(r["onchainBytecode"].as_str().unwrap()));
    assert_eq!(format!("0x{}", hex::encode(hash(&compiled))), v["code_hash"]);
    v
}

#[test]
fn candidate_changes_only_one_guard_slot_and_preserves_all_runtime_bindings() {
    let mut candidate: Vec<Value> = serde_json::from_str(CANDIDATE).unwrap();
    assert_eq!(candidate.len(), 1);
    assert_eq!(candidate[0]["other_slots"].as_array_mut().unwrap().pop().unwrap(), format!("0x{GUARD}"));
    let baseline: Vec<Value> = serde_json::from_str(BASELINE).unwrap();
    assert_eq!(baseline.len(), 431);
    assert_eq!(candidate[0], *baseline.iter().find(|p| p["contract"] == candidate[0]["contract"]).unwrap());
    assert_eq!(
        hex::encode(Sha256::digest(BASELINE.as_bytes())),
        "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468"
    );
}

#[test]
fn take_complete_source_reconstructs_runtime_and_matches_independent_pinned_repository() {
    let v = source(include_str!("fixtures/bsc-exclusions-20260928/take-source.json"));
    assert_eq!(
        bytes(v["address"].as_str().unwrap()),
        bytes(case("take")["layout"]["proxy"]["implementation"].as_str().unwrap())
    );
    assert_eq!(v["sources"].as_object().unwrap().len(), 23);
    assert_eq!(v["runtime"]["transformations"].as_array().unwrap().len(), 3);
    assert_eq!(
        v["runtime"]["transformationValues"]["immutables"]["650"],
        "0x000000000000000000000000a9709c4caf6e3fb5fd62b11b53b3f8b393c8927d"
    );
    assert_eq!(v["code_hash"], case("take")["layout"]["proxy"]["code_hash"]);
    // SHA-256 of independent raw GitHub files at the pins in the research note.
    for (path, sha) in [
        ("src/TokenImpl.sol", "0b27fe000873cd8a143ccdaacdb02e097838150d922acfa8d5e01c9bf4a14ebd"),
        (
            "lib/openzeppelin-contracts-upgradeable/contracts/utils/ReentrancyGuardUpgradeable.sol",
            "11b45418823fd6e961d8db5cf5c2795b08f51f88a66f8e37a38fcd052ee711a4",
        ),
        (
            "lib/openzeppelin-contracts-upgradeable/contracts/token/ERC20/ERC20Upgradeable.sol",
            "88081920a6d72c9b610054d4765b3a048e61faca8d19591ac0f3aa8cc7f056d4",
        ),
    ] {
        assert_eq!(hex::encode(Sha256::digest(v["sources"][path]["content"].as_str().unwrap().as_bytes())), sha);
    }
    let mut inner = hash(b"openzeppelin.storage.ReentrancyGuard");
    for byte in inner.iter_mut().rev() {
        let (value, carry) = byte.overflowing_sub(1);
        *byte = value;
        if !carry {
            break;
        }
    }
    let mut slot = hash(&inner);
    slot[31] = 0;
    assert_eq!(slot.as_slice(), bytes(GUARD));
}

#[test]
fn all_three_captured_original_refusals_are_preserved() {
    for case in cases() {
        let b = block(&case);
        assert_eq!(
            project(&b, &layout::parse(&json!([case["layout"]]).to_string()).unwrap())
                .unwrap_err()
                .to_string(),
            case["original_refusal"]
        );
    }
}

#[test]
fn take_captured_mint_preserves_delegate_storage_context_and_exact_balance() {
    let (case, b, layouts) = take();
    let call = b.transaction_traces[0].calls.iter().find(|c| c.index == 30).unwrap();
    assert_eq!(call.address, bytes(case["layout"]["proxy"]["implementation"].as_str().unwrap()));
    assert!(call.storage_changes.iter().all(|w| w.address == bytes(case["contract"].as_str().unwrap())));
    let output = project(&b, &layouts).unwrap();
    assert_eq!(output.balances.len(), 1);
    assert_eq!(output.balances[0].contract, Some(bytes(case["contract"].as_str().unwrap())));
    assert_eq!(output.balances[0].address, bytes("93b996f91e60502af0c9a4d7d94a8668257d4800"));
    assert_eq!(output.balances[0].amount, "100000000000000000000");
    let writes: Vec<_> = call.storage_changes.iter().filter(|w| w.key == bytes(GUARD)).collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(
        (
            writes[0].old_value[31],
            writes[0].new_value[31],
            writes[1].old_value[31],
            writes[1].new_value[31]
        ),
        (1, 2, 2, 1)
    );
}

#[test]
fn guard_permission_does_not_admit_adjacent_storage_or_malformed_words() {
    let (_, original, layouts) = take();
    for offset in [1, 2, 255] {
        let mut b = original.clone();
        for write in &mut mint_call(&mut b).storage_changes {
            if write.key == bytes(GUARD) {
                write.key[31] = offset;
            }
        }
        assert!(project(&b, &layouts).is_err());
    }
    for field in ["key", "old", "new", "noop"] {
        let mut malformed = original.clone();
        let write = &mut mint_call(&mut malformed).storage_changes[0];
        match field {
            "key" => write.key.push(0),
            "old" => write.old_value = vec![1; 33],
            "new" => write.new_value = vec![1; 33],
            "noop" => {
                write.old_value = vec![1; 33];
                write.new_value = vec![1; 33];
            }
            _ => unreachable!(),
        }
        assert!(project(&malformed, &layouts).unwrap_err().to_string().contains("word exceeds uint256"));
        mint_call(&mut malformed).state_reverted = true;
        assert!(project(&malformed, &layouts).unwrap().balances.is_empty());
    }
}

#[test]
fn guard_is_metadata_not_a_guard_transition_or_balance_model() {
    let (_, mut b, layouts) = take();
    // other_slots proves getter independence, not EVM opcode validity. Even
    // atypical uint256 metadata values cannot fabricate a holder balance.
    for call in &mut b.transaction_traces[0].calls {
        call.storage_changes.retain(|w| w.key == bytes(GUARD));
    }
    let writes = &mut mint_call(&mut b).storage_changes;
    writes[0].old_value = vec![7];
    writes[0].new_value = vec![9];
    writes[1].old_value = vec![9];
    writes[1].new_value = vec![7];
    assert!(project(&b, &layouts).unwrap().balances.is_empty());
    assert!(project(&b, &layout::parse(&json!([case("take")["layout"]]).to_string()).unwrap()).is_err());
}

#[test]
fn malformed_persisted_beacon_words_refuse_without_rejecting_unconfigured_storage() {
    let (_, mut b, _) = take();
    let profiles: Vec<Value> = serde_json::from_str(BASELINE).unwrap();
    let profile = profiles.iter().find(|p| p["contract"] == "0x7c8d5502b544ddaf8852fc46d1174e34876d545c").unwrap();
    let layouts = layout::parse(&json!([profile]).to_string()).unwrap();
    let write = &mut mint_call(&mut b).storage_changes[0];
    write.new_value = vec![1; 33];
    // The captured TAKE effects are outside this BNC4-only configuration.
    assert!(project(&b, &layouts).unwrap().balances.is_empty());
    let write = &mut mint_call(&mut b).storage_changes[0];
    write.address = bytes(profile["beacon_proxy"]["beacon"].as_str().unwrap());
    write.key = bytes(profile["beacon_proxy"]["implementation_slot"].as_str().unwrap());
    assert!(project(&b, &layouts).unwrap_err().to_string().contains("word exceeds uint256"));
}

#[test]
fn reverted_or_failed_mint_never_emits_and_dependency_excursions_still_refuse() {
    let (case, original, layouts) = take();
    let mut failed = original.clone();
    failed.transaction_traces[0].status = eth::TransactionTraceStatus::Failed as i32;
    assert!(project(&failed, &layouts).unwrap().balances.is_empty());
    let mut reverted = original.clone();
    mint_call(&mut reverted).state_reverted = true;
    assert!(project(&reverted, &layouts).unwrap().balances.is_empty());
    let mut code = original.clone();
    mint_call(&mut code).code_changes.push(eth::CodeChange {
        address: bytes(case["layout"]["proxy"]["implementation"].as_str().unwrap()),
        old_hash: vec![1; 32],
        new_hash: vec![2; 32],
        ordinal: 4150,
        ..Default::default()
    });
    assert!(project(&code, &layouts).unwrap_err().to_string().contains("code changed"));
    let mut pointer = original;
    let call = mint_call(&mut pointer);
    for (old, new, ordinal) in [(1, 2, 4150), (2, 1, 4151)] {
        call.storage_changes.push(eth::StorageChange {
            address: bytes(case["contract"].as_str().unwrap()),
            key: bytes(case["layout"]["proxy"]["implementation_slot"].as_str().unwrap()),
            old_value: vec![old],
            new_value: vec![new],
            ordinal,
        });
    }
    assert!(project(&pointer, &layouts).unwrap_err().to_string().contains("implementation slot changed"));
}

#[test]
fn tops_refusal_binds_to_lpinfo_element_four_but_remains_unsupported() {
    let v = source(include_str!("fixtures/bsc-exclusions-20260928/tops-source.json"));
    let c = case("tops");
    assert_eq!(bytes(v["address"].as_str().unwrap()), bytes(c["contract"].as_str().unwrap()));
    assert_eq!(v["code_hash"], c["layout"]["code_hash"]);
    let storage = &v["storage_layout"];
    let lp = storage["storage"].as_array().unwrap().iter().find(|r| r["label"] == "lpInfos").unwrap();
    assert_eq!(lp["slot"], "31");
    let mapping = &storage["types"][lp["type"].as_str().unwrap()];
    assert_eq!(mapping["key"], "t_address");
    let array = &storage["types"][mapping["value"].as_str().unwrap()];
    assert_eq!(array["encoding"], "dynamic_array");
    let record = &storage["types"][array["base"].as_str().unwrap()];
    assert_eq!(record["numberOfBytes"], "96");
    assert_eq!(record["members"][0]["label"], "lpAmount");
    let mut input = [0; 64];
    input[12..32].copy_from_slice(&bytes("28ebbb1c3003a30f9e627647862dd85aa4c165ce"));
    input[63] = 31;
    let head = hash(&input);
    let mut key = hash(&head);
    key[31] += 12;
    assert_eq!(key.as_slice(), bytes(c["refused_slot"].as_str().unwrap()));
    let b = block(&c);
    assert!(b.transaction_traces[0]
        .calls
        .iter()
        .any(|call| call.keccak_preimages.values().any(|p| bytes(p) == input)));
    assert!(project(&b, &layout::parse(&json!([c["layout"]]).to_string()).unwrap()).is_err());
}
