use crate::cli::Cli;
use crate::replay::{enumerate, run, Replay};
use clap::Parser;
use prost::Message;
use serde_json::Value;
use std::fs;
use substreams_ethereum::pb::eth::v2 as eth;

fn block(number: u64, parent: Vec<u8>, changes: Vec<eth::BalanceChange>) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![number as u8; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: parent,
            state_root: vec![3; 32],
            ..Default::default()
        }),
        balance_changes: changes,
        ..Default::default()
    }
}
fn change(address: u8, ordinal: u64, old: u8, new: u8) -> eth::BalanceChange {
    eth::BalanceChange {
        address: vec![address; 20],
        old_value: Some(eth::BigInt { bytes: vec![old] }),
        new_value: Some(eth::BigInt { bytes: vec![new] }),
        ordinal,
        reason: eth::balance_change::Reason::Transfer as i32,
    }
}
fn write(dir: &std::path::Path, b: &eth::Block) {
    fs::write(dir.join(format!("{}.pb", b.number)), b.encode_to_vec()).unwrap();
}
fn replay(dir: &std::path::Path, out: &std::path::Path) -> (bool, Value) {
    let ok = run(Replay {
        blocks: vec![dir.to_path_buf()],
        producer_versions: vec![5],
        oracle: vec![],
        prototype_rows: vec![],
        network: "test".into(),
        output: out.to_path_buf(),
    })
    .unwrap_or(false);
    let report: Value = serde_json::from_str(&fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    (ok, report)
}

#[test]
fn continuity_and_clock_links_pass_across_contiguous_blocks_and_skip_gaps() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    let b10 = block(10, vec![9; 32], vec![change(4, 1, 0, 5)]);
    let b11 = block(11, b10.hash.clone(), vec![change(4, 1, 5, 7), change(6, 2, 1, 0)]);
    // Block 12 is missing; block 13's account 4 old value is not checked.
    let b13 = block(13, vec![12; 32], vec![change(4, 1, 9, 9), change(4, 2, 9, 1)]);
    for b in [&b10, &b11, &b13] {
        write(&dir, b);
    }
    let (ok, report) = replay(&dir, &tmp.path().join("out"));
    assert!(ok, "{report}");
    assert_eq!(report["status"], "replayed");
    assert_eq!(report["blocks"], 3);
    assert_eq!(report["ranges"], serde_json::json!([[10, 12], [13, 14]]));
    assert_eq!(report["continuity"]["checks"], 1);
    assert_eq!(report["continuity"]["skipped_across_gaps"], 1);
    assert_eq!(report["clock_links"]["checked"], 1);
    assert_eq!(report["rows"], 4);
    assert_eq!(report["zero_final_rows"], 1);
    assert_eq!(report["reason_matrix"]["Block"]["REASON_TRANSFER"], 4);
    assert!(report["outputs"]["rows.jsonl"].as_str().unwrap().len() == 64);
}

#[test]
fn continuity_break_and_clock_break_are_reported_as_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    let b10 = block(10, vec![9; 32], vec![change(4, 1, 0, 5)]);
    let b11 = block(11, vec![0; 32], vec![change(4, 1, 6, 7)]);
    write(&dir, &b10);
    write(&dir, &b11);
    let (ok, report) = replay(&dir, &tmp.path().join("out"));
    assert!(!ok);
    assert_eq!(report["status"], "mismatch");
    assert_eq!(report["continuity"]["mismatches"].as_array().unwrap().len(), 1);
    assert_eq!(report["clock_links"]["mismatches"].as_array().unwrap().len(), 1);
}

#[test]
fn projection_errors_are_preserved_not_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    write(&dir, &block(10, vec![9; 32], vec![change(4, 1, 0, 5), change(4, 1, 5, 6)]));
    let (ok, report) = replay(&dir, &tmp.path().join("out"));
    assert!(!ok);
    assert_eq!(report["projection_errors"].as_array().unwrap().len(), 1);
    assert!(report["projection_errors"][0]["error"].as_str().unwrap().contains("ambiguous"));
}

#[test]
fn conflicting_duplicate_captures_are_rejected_and_identical_ones_deduplicated() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    let block10 = block(10, vec![9; 32], vec![]);
    write(&a, &block10);
    write(&b, &block10);
    assert_eq!(enumerate(&[a.clone(), b.clone()]).unwrap().len(), 1);
    write(&b, &block(10, vec![8; 32], vec![]));
    assert!(enumerate(&[a, b]).unwrap_err().to_string().contains("conflicting"));
}

#[test]
fn oracle_and_prototype_fixtures_match_the_captured_block() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    fs::copy(root.join("erc20/balances/tests/fixtures/bsc-122260950.pb"), dir.join("122260950.pb")).unwrap();
    let ok = run(Replay {
        blocks: vec![dir],
        producer_versions: vec![5],
        oracle: vec![root.join("erc20/balances/tests/fixtures/bsc-122260950.json")],
        prototype_rows: vec![root.join("native/balances/tests/fixtures/bsc-122260950-prototype-native-rows.json")],
        network: "bsc".into(),
        output: tmp.path().join("out"),
    })
    .unwrap();
    assert!(ok);
    let report: Value = serde_json::from_str(&fs::read_to_string(tmp.path().join("out/report.json")).unwrap()).unwrap();
    assert_eq!(report["status"], "replayed");
    assert_eq!(report["rows"], 82);
    assert_eq!(report["oracle"][0]["status"], "match");
    assert_eq!(report["oracle"][0]["matched"], 82);
    assert_eq!(report["oracle"][0]["hash_bound"], true);
    assert_eq!(report["prototype"][0]["status"], "match");
    assert_eq!(report["prototype"][0]["matched"], 82);
    assert!(report["reason_matrix"]["Block"]["REASON_REWARD_TRANSACTION_FEE"].as_u64().unwrap() >= 1);
    assert!(report["reason_matrix"]["Tx"]["REASON_TRANSFER"].as_u64().unwrap() > 0);
}

#[test]
fn cli_requires_blocks_and_output() {
    assert!(Cli::try_parse_from(["tools", "replay", "--blocks", "x", "--output", "o"]).is_ok());
    assert!(Cli::try_parse_from(["tools", "replay", "--output", "o"]).is_err());
    assert!(Cli::try_parse_from(["tools", "replay", "--blocks", "x", "--producer-versions", "4,5", "--output", "o"]).is_ok());
}

#[test]
fn unqualified_producer_versions_are_refused_and_reported_separately() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    let b9 = block(9, vec![8; 32], vec![change(4, 1, 0, 3)]);
    let mut b10 = block(10, b9.hash.clone(), vec![change(4, 1, 3, 5)]);
    b10.ver = 4;
    // Account 4 continues from block 10's value, which was refused, not reduced.
    let b11 = block(11, b10.hash.clone(), vec![change(4, 1, 5, 7)]);
    write(&dir, &b9);
    write(&dir, &b10);
    write(&dir, &b11);
    let (ok, report) = replay(&dir, &tmp.path().join("out"));
    assert!(ok, "{report}");
    assert_eq!(report["status"], "replayed");
    assert_eq!(report["blocks"], 3);
    assert_eq!(report["replayed_blocks"], 2);
    assert_eq!(report["clock_links"]["checked"], 2);
    assert_eq!(report["unqualified_version_blocks"]["count"], 1);
    assert_eq!(report["unqualified_version_blocks"]["blocks"][0]["ver"], 4);
    assert!(report["projection_errors"].as_array().unwrap().is_empty());
    assert_eq!(report["continuity"]["checks"], 0);
    assert_eq!(report["continuity"]["skipped_across_gaps"], 1);
    assert_eq!(report["rows"], 2);
}

/// WETH9-style wrapping: the holder's WBNB balance is an ERC-20 balance and
/// the wrapper contract's BNB is native account state. Neither is the other,
/// and neither is a second native wallet balance for the holder (issue #22).
mod wrapper_backing {
    use native_balances::{changes as native_changes, parse_params};
    use prost::Message;
    use substreams::scalar::BigInt;
    use substreams_ethereum::pb::eth::v2 as eth;

    const WBNB: &str = "bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
    const FIXTURES: &str = "../../../erc20/balances/tests/fixtures/wbnb-mutations";

    fn load(name: &str) -> (eth::Block, erc20_balances::layout::VerifiedLayout) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let block = eth::Block::decode(std::fs::read(root.join(FIXTURES).join(name)).unwrap().as_slice()).unwrap();
        let layout =
            erc20_balances::layout::parse(&std::fs::read_to_string(root.join("../../../erc20/balances/tests/fixtures/bsc-reviewed-layouts.json")).unwrap())
                .unwrap()
                .into_iter()
                .find(|l| hex::encode(&l.contract) == WBNB)
                .unwrap();
        (block, layout)
    }
    fn delta(old: &str, new: &str) -> BigInt {
        new.parse::<BigInt>().unwrap() - old.parse::<BigInt>().unwrap()
    }

    #[test]
    fn deposit_moves_native_bnb_into_the_wrapper_and_erc20_units_to_the_holder() {
        let (block, layout) = load("122288015-tx18-deposit-nested-no-transfer.pb");
        let erc20 = erc20_balances::changes(&block, std::slice::from_ref(&layout)).unwrap();
        let native = native_changes(&block, &parse_params(r#"{"producer_versions":[5]}"#).unwrap()).unwrap();
        let wad = BigInt::from(100971252078042364u64);
        assert_eq!(erc20.len(), 1);
        assert_eq!(delta(&erc20[0].old_amount, &erc20[0].amount), wad);
        let wrapper = native.iter().find(|r| hex::encode(&r.address) == WBNB).unwrap();
        assert_eq!(delta(&wrapper.old_amount, &wrapper.amount), wad);
        // The wrapper is not an ERC-20 holder row; the holder's own native
        // balance nets to zero (received then forwarded the BNB it wrapped).
        assert!(!erc20.iter().any(|r| hex::encode(&r.address) == WBNB));
        let holder = native.iter().find(|r| r.address == erc20[0].address).unwrap();
        assert_eq!((holder.old_amount.as_str(), holder.records), (holder.amount.as_str(), 2));
    }

    #[test]
    fn withdrawal_releases_native_bnb_from_the_wrapper() {
        let (block, layout) = load("122288035-tx9-withdrawal-no-transfer.pb");
        let erc20 = erc20_balances::changes(&block, std::slice::from_ref(&layout)).unwrap();
        let native = native_changes(&block, &parse_params(r#"{"producer_versions":[5]}"#).unwrap()).unwrap();
        let wad = BigInt::from(69434307925719935u64);
        assert_eq!(erc20.len(), 1);
        assert_eq!(delta(&erc20[0].old_amount, &erc20[0].amount), -wad.clone());
        let wrapper = native.iter().find(|r| hex::encode(&r.address) == WBNB).unwrap();
        assert_eq!(delta(&wrapper.old_amount, &wrapper.amount), -wad);
    }

    #[test]
    fn reverted_deposit_leaves_neither_erc20_nor_wrapper_native_rows() {
        let (block, layout) = load("122288021-tx35-reverted-transaction-deposit.pb");
        assert!(erc20_balances::changes(&block, std::slice::from_ref(&layout)).unwrap().is_empty());
        let native = native_changes(&block, &parse_params(r#"{"producer_versions":[5]}"#).unwrap()).unwrap();
        assert!(!native.iter().any(|r| hex::encode(&r.address) == WBNB));
        // Only the sender's gas debit and the system fee credit persist.
        assert_eq!(native.len(), 2);
    }
}
