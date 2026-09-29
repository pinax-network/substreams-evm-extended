#![cfg(not(target_arch = "wasm32"))]
//! Original canonical snapshots plus explicitly synthetic continuations.
//! No checkpoint is moved before its observation; no runtime/SPKG qualification.
use evm_retention::{Clock, Domain, Key, Ledger, Lookup, Origin};
use prost::Message;
use proto::pb::evm::balances::v1::Balance;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use substreams_ethereum::pb::eth::v2 as eth;

struct Case {
    record: Value,
    block: eth::Block,
    canonical: Vec<Balance>,
    layouts: Vec<erc20_balances::layout::VerifiedLayout>,
}
fn bytes(value: &Value) -> Vec<u8> {
    hex::decode(value.as_str().unwrap().trim_start_matches("0x")).unwrap()
}
fn row(value: &Value) -> Balance {
    Balance {
        contract: Some(bytes(&value["contract"])),
        address: bytes(&value["address"]),
        amount: value["amount"].as_str().unwrap().into(),
    }
}
fn key(row: &Balance) -> Key {
    Key {
        contract: row.contract.clone(),
        address: row.address.clone(),
    }
}
fn clock(block: &eth::Block) -> Clock {
    Clock {
        number: block.number,
        hash: block.hash.clone(),
        parent_hash: block.header.as_ref().unwrap().parent_hash.clone(),
    }
}
fn cases() -> Vec<Case> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/typed450-offline");
    let records: Vec<Value> = serde_json::from_str(include_str!("../../tests/fixtures/typed450-offline/cases.json")).unwrap();
    records
        .into_iter()
        .map(|record| {
            assert_eq!(record["qualified"], false);
            let raw = fs::read(root.join(record["fixture"].as_str().unwrap())).unwrap();
            assert_eq!(hex::encode(Sha256::digest(&raw)), record["full_block_sha256"]);
            assert_eq!(format!("0x{}", hex::encode(erc20_balances::hash(&raw))), record["full_block_keccak256"]);
            let block = eth::Block::decode(raw.as_slice()).unwrap();
            assert_eq!(block.number, record["block"].as_u64().unwrap());
            assert_eq!(block.hash, bytes(&record["hash"]));
            let expected = fs::read(root.join(record["provenance"]["expected"].as_str().unwrap())).unwrap();
            assert_eq!(hex::encode(Sha256::digest(&expected)), record["provenance"]["expected_sha256"]);
            let reference: Value = serde_json::from_slice(&expected).unwrap();
            assert_eq!(reference["fresh_eth_call"], false);
            assert_eq!(reference["block"], record["block"]);
            assert_eq!(reference["contract"], record["contract"]);
            let canonical = reference["balances"].as_array().unwrap().iter().map(row).collect();
            let layouts = erc20_balances::layout::parse(&json!([record["layout"]]).to_string()).unwrap();
            Case {
                record,
                block,
                canonical,
                layouts,
            }
        })
        .collect()
}
fn project(case: &Case, block: &eth::Block) -> Vec<Balance> {
    erc20_balances::project(block, &case.layouts).unwrap().balances
}
fn seeded(case: &Case) -> Ledger {
    let mut ledger = Ledger::new(4, Domain::Balances);
    for row in &case.canonical {
        ledger
            .seed_checkpoint(
                key(row),
                &row.amount,
                case.block.number,
                &case.block.hash,
                case.record["provenance"]["expected"].as_str().unwrap(),
            )
            .unwrap();
    }
    ledger
}
fn quiet_successor(block: &eth::Block) -> eth::Block {
    // This identity is invented for the test, not a captured successor clock.
    let mut successor = eth::Block {
        ver: block.ver,
        detail_level: block.detail_level,
        number: block.number + 1,
        hash: vec![0xa5; 32],
        header: block.header.clone(),
        ..Default::default()
    };
    let header = successor.header.as_mut().unwrap();
    header.number = successor.number;
    header.parent_hash = block.hash.clone();
    if let Some(timestamp) = header.timestamp.as_mut() {
        timestamp.seconds += 1;
    }
    successor
}
fn assert_known(ledger: &Ledger, expected: &Balance, origin: &Origin, observed_at: u64) {
    let Lookup::Known(entry) = ledger.lookup(&key(expected)) else {
        panic!("expected known holder");
    };
    assert_eq!(entry.value, expected.amount);
    assert_eq!(&entry.origin, origin);
    assert_eq!(entry.since, observed_at);
    assert_eq!(entry.updated, observed_at);
}

#[test]
fn captured_projectors_initialize_five_holders_and_keep_six_reference_gaps_unknown() {
    let (mut known, mut gaps, mut nonzero_gaps, mut known_zeros) = (0, 0, 0, 0);
    for case in cases() {
        let rows = project(&case, &case.block);
        let expected: BTreeMap<_, _> = case.record["expected_emitted"]
            .as_array()
            .unwrap()
            .iter()
            .map(row)
            .map(|r| (key(&r), r))
            .collect();
        assert_eq!(rows.iter().map(|r| (key(r), r.clone())).collect::<BTreeMap<_, _>>(), expected);
        let mut ledger = Ledger::new(4, Domain::Balances);
        ledger.apply(&clock(&case.block), &rows).unwrap();
        let quiet = quiet_successor(&case.block);
        let quiet_rows = project(&case, &quiet);
        assert!(quiet_rows.is_empty());
        ledger.apply(&clock(&quiet), &quiet_rows).unwrap();
        for reference in &case.canonical {
            if expected.contains_key(&key(reference)) {
                assert_known(&ledger, reference, &Origin::Observed, case.block.number);
                known += 1;
                known_zeros += usize::from(reference.amount == "0");
            } else {
                assert_eq!(ledger.lookup(&key(reference)), Lookup::Unknown);
                gaps += 1;
                nonzero_gaps += usize::from(reference.amount != "0");
            }
        }
        assert_eq!(ledger.entries().len(), expected.len());
        assert_eq!(ledger.report().checkpoint_seeded_holders, 0);
    }
    assert_eq!((known, known_zeros, gaps, nonzero_gaps), (5, 1, 6, 2));
}

#[test]
fn eleven_canonical_snapshot_seeds_start_only_after_their_actual_observation_clock() {
    let (mut holders, mut zeros) = (0, 0);
    for case in cases() {
        let mut ledger = seeded(&case);
        let before = ledger.entries().clone();
        // Replaying the snapshot's own block would apply earlier facts after an
        // end-of-block checkpoint. A wrong parent is also refused atomically.
        assert!(ledger.apply(&clock(&case.block), &project(&case, &case.block)).is_err());
        let quiet = quiet_successor(&case.block);
        let mut wrong_parent = clock(&quiet);
        wrong_parent.parent_hash[0] ^= 1;
        assert!(ledger.apply(&wrong_parent, &project(&case, &quiet)).is_err());
        assert_eq!(ledger.entries(), &before);
        assert!(ledger.last().is_none());
        ledger.apply(&clock(&quiet), &project(&case, &quiet)).unwrap();
        let origin = Origin::Checkpoint {
            block: case.block.number,
            hash: case.block.hash.clone(),
            evidence: case.record["provenance"]["expected"].as_str().unwrap().into(),
        };
        for reference in &case.canonical {
            assert_known(&ledger, reference, &origin, case.block.number);
            holders += 1;
            zeros += usize::from(reference.amount == "0");
        }
        let never_observed = Key {
            contract: Some(bytes(&case.record["contract"])),
            address: vec![0x42; 20],
        };
        assert_eq!(ledger.lookup(&never_observed), Lookup::Unknown);
        assert_eq!(ledger.entries(), &before);
        assert_eq!(ledger.last(), Some(&clock(&quiet)));
    }
    assert_eq!((holders, zeros), (11, 5));
}

#[test]
fn derived_restored_allowance_continuation_neither_initializes_nor_changes_balances() {
    let case = cases().into_iter().find(|c| c.record["rank"] == 418).unwrap();
    let contract = bytes(&case.record["contract"]);
    let allowance_key = bytes(&case.record["allowance_writes"][0]["key"]);
    let mut derived = case.block.clone();
    let identity = quiet_successor(&case.block);
    derived.number = identity.number;
    derived.hash = identity.hash;
    derived.header = identity.header;
    // Keep original call/preimage context, but this filtered, renumbered block
    // is explicitly synthetic. Only the captured round-trip allowance remains.
    for call in derived
        .system_calls
        .iter_mut()
        .chain(derived.transaction_traces.iter_mut().flat_map(|t| &mut t.calls))
    {
        call.storage_changes.retain(|s| s.address == contract && s.key == allowance_key);
    }
    let mut writes: Vec<_> = derived
        .transaction_traces
        .iter()
        .filter(|t| t.status() == eth::TransactionTraceStatus::Succeeded)
        .flat_map(|t| &t.calls)
        .chain(&derived.system_calls)
        .filter(|c| !c.state_reverted)
        .flat_map(|c| &c.storage_changes)
        .collect();
    writes.sort_by_key(|s| s.ordinal);
    assert_eq!(writes.len(), 2);
    for (write, recorded) in writes.iter().zip(case.record["allowance_writes"].as_array().unwrap()) {
        assert_eq!(write.old_value, bytes(&recorded["old"]));
        assert_eq!(write.new_value, bytes(&recorded["new"]));
        assert_eq!(write.ordinal, recorded["ordinal"].as_u64().unwrap());
    }
    assert_ne!(writes[0].old_value, writes[0].new_value);
    assert_eq!(writes[0].new_value, writes[1].old_value);
    assert_eq!(writes[1].new_value, writes[0].old_value);
    let rows = project(&case, &derived);
    assert!(rows.is_empty());
    let mut cold = Ledger::new(4, Domain::Balances);
    cold.apply(&clock(&case.block), &project(&case, &case.block)).unwrap();
    let mut checkpoint = seeded(&case);
    for ledger in [&mut cold, &mut checkpoint] {
        let before = ledger.entries().clone();
        ledger.apply(&clock(&derived), &rows).unwrap();
        assert_eq!(ledger.entries(), &before);
    }
    let self_key = Key {
        contract: Some(contract.clone()),
        address: contract,
    };
    assert_eq!(cold.lookup(&self_key), Lookup::Unknown);
    let Lookup::Known(entry) = checkpoint.lookup(&self_key) else {
        panic!("explicit snapshot seed missing");
    };
    assert_eq!(entry.value, "20390672839");
    assert_eq!(cold.entries().len(), 3);
    assert_eq!(checkpoint.entries().len(), 5);
}
