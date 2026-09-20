//! Non-Transfer balance mutations of the WETH9-style WBNB contract, captured
//! from saved BSC Extended blocks (issue #22). Each fixture keeps the block
//! header and identity plus one unmodified transaction trace; provenance is in
//! `tests/fixtures/wbnb-mutations/cases.json`.
//!
//! `deposit()` and `withdraw()` emit only `Deposit`/`Withdrawal` and write the
//! holder's balance word directly, so an event-driven candidate discovery that
//! keys on `Transfer` would miss these holders. The storage mapper emits them
//! from the persisted write and the verified Keccak preimage, with the holder
//! resolved from the write itself rather than from the transaction sender,
//! the caller, or a log topic.
use super::*;
use prost::Message;
use sha2::Digest;

const WBNB: &str = "bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
const DEPOSIT_TOPIC: &str = "e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c";
const WITHDRAWAL_TOPIC: &str = "7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65";
const TRANSFER_TOPIC: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

const DEPOSIT_NESTED: &[u8] = include_bytes!("../tests/fixtures/wbnb-mutations/122288015-tx18-deposit-nested-no-transfer.pb");
const DEPOSIT_WITHDRAW_ZERO: &[u8] = include_bytes!("../tests/fixtures/wbnb-mutations/122288032-tx47-deposit-withdraw-zero-final.pb");
const WITHDRAWAL: &[u8] = include_bytes!("../tests/fixtures/wbnb-mutations/122288035-tx9-withdrawal-no-transfer.pb");
const REVERTED_TX: &[u8] = include_bytes!("../tests/fixtures/wbnb-mutations/122288021-tx35-reverted-transaction-deposit.pb");
const REVERTED_CHILD: &[u8] = include_bytes!("../tests/fixtures/wbnb-mutations/122288007-tx22-succeeded-with-reverted-child-write.pb");

fn wbnb_layout() -> VerifiedLayout {
    layout::parse(include_str!("../tests/fixtures/bsc-reviewed-layouts.json"))
        .unwrap()
        .into_iter()
        .find(|l| hex::encode(&l.contract) == WBNB)
        .unwrap()
}
fn decode(bytes: &[u8]) -> eth::Block {
    let block = eth::Block::decode(bytes).unwrap();
    assert_eq!(block.ver, 5);
    assert_eq!(block.transaction_traces.len(), 1);
    block
}
struct WbnbLogs {
    deposits: Vec<(String, String)>,
    withdrawals: Vec<(String, String)>,
    transfers: usize,
    in_reverted_frames: usize,
}
fn wbnb_logs(block: &eth::Block) -> WbnbLogs {
    let wbnb = hex_bytes(WBNB).unwrap();
    let mut out = WbnbLogs {
        deposits: vec![],
        withdrawals: vec![],
        transfers: 0,
        in_reverted_frames: 0,
    };
    for call in block.transaction_traces.iter().flat_map(|t| &t.calls) {
        for log in call.logs.iter().filter(|l| l.address == wbnb) {
            if call.state_reverted {
                out.in_reverted_frames += 1;
            }
            let topic = hex::encode(&log.topics[0]);
            let account = hex::encode(&log.topics[1][12..]);
            let wad = amount(&log.data).unwrap();
            if topic == DEPOSIT_TOPIC {
                out.deposits.push((account, wad));
            } else if topic == WITHDRAWAL_TOPIC {
                out.withdrawals.push((account, wad));
            } else if topic == TRANSFER_TOPIC {
                out.transfers += 1;
            }
        }
    }
    out
}
fn delta(row: &Change) -> BigInt {
    row.amount.parse::<BigInt>().unwrap() - row.old_amount.parse::<BigInt>().unwrap()
}
fn addresses(rows: &[Change]) -> Vec<String> {
    rows.iter().map(|r| hex::encode(&r.address)).collect()
}

#[test]
fn nested_contract_deposit_without_transfer_emits_the_depositor_from_storage() {
    let block = decode(DEPOSIT_NESTED);
    let tx = &block.transaction_traces[0];
    let logs = wbnb_logs(&block);
    assert_eq!(
        logs.deposits,
        vec![("278d858f05b94576c1e6f73285886876ff6ef8d2".into(), "100971252078042364".into())]
    );
    assert!(logs.withdrawals.is_empty() && logs.transfers == 0);
    // The depositor is the called contract five frames deep, not the sender.
    assert_eq!(hex::encode(&tx.from), "3a001afce9d414abd74885d29b646a6eb33a4ed4");
    assert_eq!(hex::encode(&tx.to), "278d858f05b94576c1e6f73285886876ff6ef8d2");
    let rows = changes(&block, &[wbnb_layout()]).unwrap();
    assert_eq!(addresses(&rows), vec!["278d858f05b94576c1e6f73285886876ff6ef8d2"]);
    assert_eq!(
        (&*rows[0].old_amount, &*rows[0].amount, rows[0].ordinal),
        ("985327611293467127043", "985428582545545169407", 1158)
    );
    assert_eq!(delta(&rows[0]).to_string(), logs.deposits[0].1);
    let events = project(&block, &[wbnb_layout()]).unwrap();
    assert_eq!(events.balances.len(), 1);
    assert_eq!(events.balances[0].amount, "985428582545545169407");
    assert_eq!(events.balances[0].contract.as_deref().map(hex::encode).as_deref(), Some(WBNB));
}

#[test]
fn deposit_then_full_withdrawal_emits_a_known_zero_not_an_absence() {
    let block = decode(DEPOSIT_WITHDRAW_ZERO);
    let logs = wbnb_logs(&block);
    let router = "2d9d6e538bd3f22323932782aaf89446cacaf9d3";
    assert_eq!(logs.deposits, vec![(router.into(), "128305530000000".into())]);
    assert_eq!(logs.withdrawals, vec![(router.into(), "128305530000000".into())]);
    assert_eq!(logs.transfers, 0);
    let rows = changes(&block, &[wbnb_layout()]).unwrap();
    assert_eq!(addresses(&rows), vec![router]);
    assert_eq!((&*rows[0].old_amount, &*rows[0].amount, rows[0].ordinal), ("0", "0", 2072));
    let events = project(&block, &[wbnb_layout()]).unwrap();
    assert_eq!(events.balances.len(), 1);
    assert_eq!(events.balances[0].amount, "0");
}

#[test]
fn withdrawal_without_transfer_debits_the_storage_holder() {
    let block = decode(WITHDRAWAL);
    let tx = &block.transaction_traces[0];
    let logs = wbnb_logs(&block);
    let holder = "4e7ed91e702ef2ff0c58e251c6e20d1dc1e31a5f";
    assert!(logs.deposits.is_empty() && logs.transfers == 0);
    assert_eq!(logs.withdrawals, vec![(holder.into(), "69434307925719935".into())]);
    assert_ne!(hex::encode(&tx.from), holder);
    let rows = changes(&block, &[wbnb_layout()]).unwrap();
    assert_eq!(addresses(&rows), vec![holder]);
    assert_eq!(
        (&*rows[0].old_amount, &*rows[0].amount, rows[0].ordinal),
        ("47105487097897227928", "47036052789971507993", 3008)
    );
    assert_eq!((-delta(&rows[0])).to_string(), logs.withdrawals[0].1);
}

#[test]
fn reverted_transaction_with_deposit_event_and_writes_emits_nothing() {
    let block = decode(REVERTED_TX);
    let tx = &block.transaction_traces[0];
    assert_eq!(tx.status, eth::TransactionTraceStatus::Reverted as i32);
    let logs = wbnb_logs(&block);
    // The event and the storage writes exist in the trace but never persisted.
    assert_eq!(logs.deposits.len(), 1);
    assert_eq!(logs.transfers, 1);
    assert_eq!(logs.in_reverted_frames, 2);
    assert!(tx
        .calls
        .iter()
        .any(|c| c.state_reverted && c.storage_changes.iter().any(|s| hex::encode(&s.address) == WBNB)));
    assert!(changes(&block, &[wbnb_layout()]).unwrap().is_empty());
    assert!(project(&block, &[wbnb_layout()]).unwrap().balances.is_empty());
}

#[test]
fn succeeded_transaction_ignores_reverted_child_writes_and_resolves_owners_from_preimages() {
    let block = decode(REVERTED_CHILD);
    let tx = &block.transaction_traces[0];
    assert_eq!(tx.status, eth::TransactionTraceStatus::Succeeded as i32);
    let wbnb = hex_bytes(WBNB).unwrap();
    let reverted_writes: Vec<_> = tx
        .calls
        .iter()
        .filter(|c| c.state_reverted)
        .flat_map(|c| &c.storage_changes)
        .filter(|s| s.address == wbnb)
        .collect();
    assert_eq!(reverted_writes.iter().map(|s| s.ordinal).collect::<Vec<_>>(), vec![778, 779]);
    // The first WBNB call is made through Permit2, not by the sender; the
    // debited holder is neither the sender nor the caller.
    let permit2 = tx.calls.iter().find(|c| c.address == wbnb).unwrap();
    assert_eq!(hex::encode(&permit2.caller), "000000000022d473030f116ddee9f6b43ac78ba3");
    assert_eq!(hex::encode(&tx.from), "d20030e3d5631e63e26e2adcbc13bdb2275c9a07");
    let rows = changes(&block, &[wbnb_layout()]).unwrap();
    assert_eq!(
        addresses(&rows),
        vec![
            "5e2dc2eeab4f1ab9ea12abcf1321d2a8b34f584a",
            "b8a3a3789b351ed3a8da1ba69f859a2905965d5b",
            "e2588c219697f520757b82f5b0119d72bddc0e13"
        ]
    );
    // Had the reverted writes at 778/779 been applied, the later persisted
    // writes at 802/803 would have failed the old/new continuity check.
    assert_eq!((&*rows[0].old_amount, &*rows[0].amount, rows[0].ordinal), ("0", "0", 802));
    assert_eq!(
        (&*rows[1].old_amount, &*rows[1].amount, rows[1].ordinal),
        ("56916357922176472462", "56996762823875887205", 803)
    );
    assert_eq!(
        (&*rows[2].old_amount, &*rows[2].amount, rows[2].ordinal),
        ("287579705497612799198", "287499300595913384455", 753)
    );
    assert_eq!(delta(&rows[1]), -delta(&rows[2]));
    assert!(!addresses(&rows).contains(&hex::encode(&tx.from)));
    assert!(!addresses(&rows).contains(&"000000000022d473030f116ddee9f6b43ac78ba3".to_string()));
}

#[test]
fn every_fixture_carries_its_recorded_identity() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/wbnb-mutations/cases.json")).unwrap();
    let fixtures = [
        ("122288015-tx18-deposit-nested-no-transfer.pb", DEPOSIT_NESTED),
        ("122288032-tx47-deposit-withdraw-zero-final.pb", DEPOSIT_WITHDRAW_ZERO),
        ("122288035-tx9-withdrawal-no-transfer.pb", WITHDRAWAL),
        ("122288021-tx35-reverted-transaction-deposit.pb", REVERTED_TX),
        ("122288007-tx22-succeeded-with-reverted-child-write.pb", REVERTED_CHILD),
    ];
    let listed = cases["cases"].as_array().unwrap();
    assert_eq!(listed.len(), fixtures.len());
    for (file, bytes) in fixtures {
        let case = listed.iter().find(|c| c["file"] == file).unwrap();
        let block = decode(bytes);
        assert_eq!(case["block"], block.number);
        assert_eq!(case["block_hash"], format!("0x{}", hex::encode(&block.hash)));
        assert_eq!(case["transaction"], format!("0x{}", hex::encode(&block.transaction_traces[0].hash)));
        assert_eq!(case["transaction_index"], block.transaction_traces[0].index);
        assert_eq!(case["fixture_sha256"], hex::encode(sha2::Sha256::digest(bytes)));
    }
}
