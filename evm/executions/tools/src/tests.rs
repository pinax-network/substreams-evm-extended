use crate::replay::{run, storage_context, Replay};
use prost::Message;
use serde_json::Value;
use std::fs;
use std::path::Path;
use substreams_ethereum::pb::eth::v2 as eth;

const FULL_BLOCK: &[u8] = include_bytes!("../../../../erc20/balances/tests/fixtures/bsc-122260950.pb");

fn replay(blocks: &[eth::Block]) -> (bool, Value) {
    let input = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    for block in blocks {
        fs::write(input.path().join(format!("{}.pb", block.number)), block.encode_to_vec()).unwrap();
    }
    let passed = run(Replay {
        blocks: vec![input.path().to_path_buf()],
        producer_versions: vec![4, 5],
        chain_id: 56,
        network: "bsc".into(),
        output: output.path().to_path_buf(),
    })
    .unwrap();
    let report = serde_json::from_str(&fs::read_to_string(output.path().join("report.json")).unwrap()).unwrap();
    (passed, report)
}
fn full_block() -> eth::Block {
    eth::Block::decode(FULL_BLOCK).unwrap()
}

#[test]
fn the_captured_block_replays_cleanly_and_delegate_frames_write_their_callers_storage() {
    let (passed, report) = replay(&[full_block()]);
    assert!(passed, "{report:#}");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["blocks"]["projected"], 1);
    assert_eq!(report["blocks"]["projection_errors"], 0);
    assert_eq!(report["determinism"]["mismatches"], 0);
    assert_eq!(report["storage_context"]["violations"], 0);
    // The captured block exercises delegatecall storage, the case the rule is for.
    assert!(report["storage_context"]["writes_by_call_type"]["DELEGATE"].as_u64().unwrap() > 0);
    let capabilities = &report["capabilities_by_producer_version"]["5"];
    assert_eq!(capabilities["blocks"], 1);
    assert_eq!(capabilities["delegated_frames_eip7702"], 22);
    assert_eq!(capabilities["blob_transactions"], 1);
    assert!(capabilities["system_calls"].as_u64().unwrap() >= 1);
    // Persisted trace logs equal receipt logs for every transaction.
    assert_eq!(report["rows"]["receipt_logs"], 253);
    assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/replay.rs").exists());
}

#[test]
fn a_delegate_frame_writing_its_own_address_is_a_context_violation() {
    let mut block = full_block();
    let call = block
        .transaction_traces
        .iter_mut()
        .flat_map(|tx| tx.calls.iter_mut())
        .find(|c| c.call_type == eth::CallType::Delegate as i32 && !c.storage_changes.is_empty())
        .expect("the captured block has a delegate frame with storage writes");
    assert_eq!(storage_context(call), call.caller.as_slice());
    let own = call.address.clone();
    call.storage_changes[0].address = own;
    let (passed, report) = replay(&[block]);
    assert!(!passed);
    assert_eq!(report["storage_context"]["violations"], 1);
    assert_eq!(report["storage_context"]["violation_examples"][0]["call_type"], "DELEGATE");
}

#[test]
fn receipt_tampering_is_a_projection_error_and_unqualified_versions_are_refused() {
    let mut tampered = full_block();
    let tx = tampered
        .transaction_traces
        .iter_mut()
        .find(|t| t.receipt.as_ref().is_some_and(|r| !r.logs.is_empty()))
        .unwrap();
    tx.receipt.as_mut().unwrap().logs[0].data.push(0);
    let (passed, report) = replay(&[tampered]);
    assert!(!passed);
    assert_eq!(report["blocks"]["projection_errors"], 1);
    assert!(report["blocks"]["projection_error_examples"][0]["error"]
        .as_str()
        .unwrap()
        .contains("receipt logs disagree"));
    let mut v3 = full_block();
    v3.ver = 3;
    let (passed, report) = replay(&[v3]);
    assert!(passed, "a refused version is reported, not failed: {report:#}");
    assert_eq!(report["blocks"]["refused_unqualified_producer_version"]["3"], 1);
    assert_eq!(report["blocks"]["projected"], 0);
    assert_eq!(report["capabilities_by_producer_version"]["3"]["blocks"], 1);
}
