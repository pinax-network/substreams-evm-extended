use super::*;
use buffa::Message;
use serde_json::Value;

const FIXTURE_BLOCK: &[u8] = include_bytes!("../../../erc20/balances/tests/fixtures/bsc-122260950.pb");
const FIXTURE_ORACLE: &str = include_str!("../../../erc20/balances/tests/fixtures/bsc-122260950.json");
const PROTOTYPE_ROWS: &str = include_str!("../tests/fixtures/bsc-122260950-prototype-native-rows.json");
const SYSTEM_FEE_ADDRESS: &str = "fffffffffffffffffffffffffffffffffffffffe";

fn block() -> eth::Block {
    eth::Block {
        ver: 5,
        number: 122260950,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended.into(),
        header: buffa::MessageField::some(eth::BlockHeader {
            number: 122260950,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn tx(call: eth::Call) -> eth::TransactionTrace {
    eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded.into(),
        calls: vec![call],
        ..Default::default()
    }
}
fn big(v: u64) -> Option<eth::BigInt> {
    Some(eth::BigInt {
        bytes: v.to_be_bytes().to_vec(),
    })
}
fn change(address: u8, ordinal: u64, old: Option<u64>, new: Option<u64>) -> eth::BalanceChange {
    eth::BalanceChange {
        address: vec![address; 20],
        old_value: old.and_then(big).into(),
        new_value: new.and_then(big).into(),
        ordinal,
        reason: eth::balance_change::Reason::Transfer.into(),
    }
}
fn row(rows: &[Change], address: u8) -> &Change {
    rows.iter().find(|r| r.address == vec![address; 20]).unwrap()
}

#[test]
fn captured_block_matches_all_82_historical_native_rpc_balances() {
    let b = eth::Block::decode_from_slice(FIXTURE_BLOCK).unwrap();
    let oracle: Value = serde_json::from_str(FIXTURE_ORACLE).unwrap();
    assert_eq!(format!("0x{}", hex::encode(&b.hash)), oracle["block_hash"]);
    assert_eq!(b.number, 122260950);
    assert_eq!(b.ver, 5);
    let native: Vec<&Value> = oracle["rpc_checks"].as_array().unwrap().iter().filter(|r| r["contract"] == "").collect();
    assert_eq!(native.len(), 82);
    let events = project(&b).unwrap();
    assert_eq!(events.balances.len(), 82);
    for expected in native {
        let address = hex::decode(expected["address"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        let actual = events.balances.iter().find(|b| b.address == address).unwrap();
        assert_eq!(actual.contract, None);
        assert_eq!(actual.amount, expected["balance"].as_str().unwrap(), "address {}", expected["address"]);
    }
    // Sorted by account for deterministic sink rows.
    assert!(events.balances.windows(2).all(|w| w[0].address < w[1].address));
}

#[test]
fn captured_block_matches_the_historical_prototype_old_new_and_ordinal_rows() {
    let b = eth::Block::decode_from_slice(FIXTURE_BLOCK).unwrap();
    let prototype: Value = serde_json::from_str(PROTOTYPE_ROWS).unwrap();
    assert_eq!(prototype["block"]["number"], 122260950);
    assert_eq!(format!("0x{}", hex::encode(&b.hash)), prototype["block"]["hash"]);
    assert_eq!(
        format!("0x{}", hex::encode(&b.header.as_option().unwrap().parent_hash)),
        prototype["block"]["parent_hash"]
    );
    let rows = changes(&b).unwrap();
    let expected = prototype["native_rows"].as_array().unwrap();
    assert_eq!(expected.len(), 82);
    assert_eq!(rows.len(), 82);
    for e in expected {
        let address = hex::decode(e["address"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        let actual = rows.iter().find(|r| r.address == address).unwrap();
        assert_eq!(actual.old_amount, e["old_amount"].as_str().unwrap(), "{}", e["address"]);
        assert_eq!(actual.amount, e["amount"].as_str().unwrap(), "{}", e["address"]);
        assert_eq!(actual.ordinal, e["ordinal"].as_u64().unwrap(), "{}", e["address"]);
    }
}

#[test]
fn bsc_fee_reset_wins_over_the_earlier_transaction_fee_credit() {
    // Transaction ordinal 3237 credits the system fee account; the block-level
    // record at ordinal 3244 resets it to zero. Array order puts the block-level
    // record after every transaction, so only ordinal order reproduces RPC.
    let b = eth::Block::decode_from_slice(FIXTURE_BLOCK).unwrap();
    let system = hex::decode(SYSTEM_FEE_ADDRESS).unwrap();
    let credit = b
        .transaction_traces
        .iter()
        .flat_map(|t| &t.calls)
        .flat_map(|c| &c.balance_changes)
        .find(|c| c.address == system && c.ordinal == 3237)
        .unwrap();
    assert_eq!(
        amount(&value(credit.new_value.as_option().map(|v| v.bytes.as_slice())).unwrap()),
        "1675456549641341"
    );
    let reset = b.balance_changes.iter().find(|c| c.address == system).unwrap();
    assert_eq!(reset.ordinal, 3244);
    // The BSC v5 producer records the block-level reset under the ordinary
    // fee-reward reason; REWARD_FEE_RESET is not what this fixture carries.
    assert_eq!(reset.reason, eth::balance_change::Reason::RewardTransactionFee);
    assert_eq!(amount(&value(reset.new_value.as_option().map(|v| v.bytes.as_slice())).unwrap()), "0");
    let rows = changes(&b).unwrap();
    let row = rows.iter().find(|r| r.address == system).unwrap();
    assert_eq!((&*row.old_amount, &*row.amount, row.ordinal), ("0", "0", 3244));
    assert!(row.records >= 2 && row.first_ordinal < 3237);
    // Known-zero finals, precompile-style and burn-looking accounts are emitted.
    let events = project(&b).unwrap();
    assert!(events.balances.iter().any(|b| b.address == system && b.amount == "0"));
    assert!(events
        .balances
        .iter()
        .any(|b| hex::encode(&b.address) == "0000000000000000000000000000000000001000"));
    assert!(events
        .balances
        .iter()
        .any(|b| hex::encode(&b.address) == "000000000000000000000000000000000000dead"));
}

#[test]
fn captured_failed_setcode_transactions_keep_only_gas_effects() {
    for bytes in [
        include_bytes!("../../../erc20/balances/tests/fixtures/bsc-121114122-failed-setcode.pb").as_slice(),
        include_bytes!("../../../erc20/balances/tests/fixtures/bsc-121114153-failed-setcode.pb").as_slice(),
    ] {
        let trace = eth::TransactionTrace::decode_from_slice(bytes).unwrap();
        assert!(persist::is_failed(&trace));
        let root = &trace.calls[0];
        // Independent reduction of the root call's gas-reason records only.
        let mut expected: BTreeMap<Vec<u8>, (String, String)> = BTreeMap::new();
        let mut gas: Vec<_> = root.balance_changes.iter().filter(|c| persist::is_gas_reason(c)).collect();
        gas.sort_by_key(|c| c.ordinal);
        for c in gas {
            let old = amount(&value(c.old_value.as_option().map(|v| v.bytes.as_slice())).unwrap());
            let new = amount(&value(c.new_value.as_option().map(|v| v.bytes.as_slice())).unwrap());
            if old == new {
                continue;
            }
            expected.entry(c.address.clone()).or_insert((old, String::new())).1 = new;
        }
        let mut b = block();
        b.transaction_traces = vec![trace];
        let rows = changes(&b).unwrap();
        assert!(!rows.is_empty());
        assert_eq!(rows.len(), expected.len());
        for r in rows {
            assert_eq!((r.old_amount, r.amount), expected[&r.address]);
        }
    }
}

#[test]
fn takes_last_persisted_value_by_ordinal_and_preserves_first_old() {
    let mut b = block();
    b.balance_changes = vec![change(4, 30, Some(9), Some(0)), change(4, 10, Some(5), Some(9))];
    let rows = changes(&b).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((&*rows[0].old_amount, &*rows[0].amount), ("5", "0"));
    assert_eq!((rows[0].first_ordinal, rows[0].ordinal, rows[0].records), (10, 30, 2));
    let events = project(&b).unwrap();
    assert_eq!(events.balances[0].amount, "0");
    assert_eq!(events.balances[0].contract, None);
}

#[test]
fn net_zero_changing_account_is_emitted_but_individual_noop_is_not() {
    let mut b = block();
    b.balance_changes = vec![
        change(4, 10, Some(0), Some(5)),
        change(4, 20, Some(5), Some(0)),
        change(6, 30, Some(7), Some(7)),
    ];
    let rows = changes(&b).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((&*rows[0].old_amount, &*rows[0].amount, rows[0].records), ("0", "0", 2));
    assert_eq!(project(&b).unwrap().balances.len(), 1);
}

#[test]
fn missing_value_messages_inside_an_observed_record_encode_zero() {
    let mut b = block();
    b.balance_changes = vec![change(4, 10, None, Some(7)), change(5, 11, Some(7), None)];
    let rows = changes(&b).unwrap();
    assert_eq!((&*row(&rows, 4).old_amount, &*row(&rows, 4).amount), ("0", "7"));
    assert_eq!((&*row(&rows, 5).old_amount, &*row(&rows, 5).amount), ("7", "0"));
    // A record whose absent and zero values agree is a no-op and never a row.
    b.balance_changes = vec![change(6, 12, None, Some(0))];
    assert!(changes(&b).unwrap().is_empty());
}

#[test]
fn zero_precompile_and_burn_looking_accounts_are_ordinary_accounts() {
    let mut b = block();
    let mut zero = change(0, 10, Some(1), Some(2));
    zero.address = vec![0; 20];
    let mut precompile = change(0, 11, Some(1), Some(2));
    precompile.address = hex::decode("0000000000000000000000000000000000000001").unwrap();
    let mut dead = change(0, 12, Some(1), Some(0));
    dead.address = hex::decode("000000000000000000000000000000000000dead").unwrap();
    b.balance_changes = vec![zero, precompile, dead];
    let events = project(&b).unwrap();
    assert_eq!(events.balances.len(), 3);
    assert_eq!(events.balances[0].address, vec![0; 20]);
    assert_eq!(events.balances[2].amount, "0");
}

#[test]
fn ambiguous_or_discontinuous_producer_data_fails_the_block() {
    let mut b = block();
    b.balance_changes = vec![change(4, 1, Some(0), Some(1)), change(4, 1, Some(1), Some(2))];
    assert!(changes(&b).unwrap_err().to_string().contains("ambiguous"));
    b.balance_changes = vec![change(4, 1, Some(0), Some(1)), change(4, 2, Some(3), Some(4))];
    assert!(changes(&b).unwrap_err().to_string().contains("discontinuous"));
    b.balance_changes = vec![change(4, 0, Some(0), Some(1))];
    assert!(changes(&b).unwrap_err().to_string().contains("ordinal"));
    let mut short = change(4, 1, Some(0), Some(1));
    short.address = vec![4; 19];
    b.balance_changes = vec![short];
    assert!(changes(&b).unwrap_err().to_string().contains("address"));
    let mut wide = change(4, 1, Some(0), Some(1));
    wide.new_value = buffa::MessageField::some(eth::BigInt { bytes: vec![1; 33] });
    b.balance_changes = vec![wide];
    assert!(changes(&b).unwrap_err().to_string().contains("uint256"));
}

#[test]
fn preserves_uint256_max_and_leading_zero_encodings() {
    let mut b = block();
    let mut max = change(4, 1, Some(0), Some(1));
    max.new_value = buffa::MessageField::some(eth::BigInt { bytes: vec![255; 32] });
    let mut padded = change(5, 2, Some(0), Some(1));
    padded.new_value = buffa::MessageField::some(eth::BigInt { bytes: vec![0, 0, 0, 9] });
    b.balance_changes = vec![max, padded];
    let rows = changes(&b).unwrap();
    assert_eq!(
        row(&rows, 4).amount,
        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
    );
    assert_eq!(row(&rows, 5).amount, "9");
}

#[test]
fn reverted_frames_failed_transactions_and_system_calls_follow_persistence_rules() {
    let mut b = block();
    let mut reverted = eth::Call {
        index: 1,
        state_reverted: true,
        balance_changes: vec![change(7, 5, Some(0), Some(9))],
        ..Default::default()
    };
    let root = eth::Call {
        index: 0,
        balance_changes: vec![change(4, 3, Some(0), Some(1))],
        ..Default::default()
    };
    let mut ok = tx(root);
    ok.calls.push(std::mem::take(&mut reverted));
    let mut failed = eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Reverted.into(),
        calls: vec![eth::Call {
            state_reverted: true,
            balance_changes: vec![
                eth::BalanceChange {
                    reason: eth::balance_change::Reason::GasBuy.into(),
                    ..change(8, 20, Some(100), Some(90))
                },
                change(9, 21, Some(0), Some(50)),
                eth::BalanceChange {
                    reason: eth::balance_change::Reason::RewardTransactionFee.into(),
                    ..change(10, 22, Some(0), Some(10))
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    failed.from = vec![8; 20];
    b.transaction_traces = vec![ok, failed];
    b.system_calls = vec![
        eth::Call {
            balance_changes: vec![change(11, 40, Some(1), Some(2))],
            ..Default::default()
        },
        eth::Call {
            state_reverted: true,
            balance_changes: vec![change(12, 41, Some(1), Some(2))],
            ..Default::default()
        },
    ];
    b.balance_changes = vec![change(13, 50, Some(0), Some(3))];
    let rows = changes(&b).unwrap();
    let accounts: Vec<u8> = rows.iter().map(|r| r.address[0]).collect();
    assert_eq!(accounts, vec![4, 8, 10, 11, 13]);
    assert_eq!(row(&rows, 8).amount, "90");
}

#[test]
fn value_bearing_calls_without_persisted_records_are_not_balances() {
    let mut b = block();
    b.transaction_traces = vec![tx(eth::Call {
        call_type: eth::CallType::Delegate.into(),
        value: big(5).into(),
        caller: vec![4; 20],
        address: vec![5; 20],
        ..Default::default()
    })];
    assert!(project(&b).unwrap().balances.is_empty());
}

#[test]
fn incomplete_blocks_are_rejected() {
    let mut b = block();
    b.ver = 99;
    b.detail_level = eth::block::DetailLevel::DetaillevelBase.into();
    assert!(project(&b).unwrap_err().to_string().contains("Extended blocks required"));
    b = block();
    b.header = buffa::MessageField::none();
    assert!(project(&b).is_err());
    b = block();
    b.header.as_option_mut().unwrap().number = 1;
    assert!(project(&b).is_err());
    b = block();
    b.hash = vec![1; 31];
    assert!(project(&b).is_err());
    b = block();
    b.number = 0;
    b.header.as_option_mut().unwrap().number = 0;
    assert!(project(&b).unwrap_err().to_string().contains("genesis"));
    b = block();
    b.transaction_traces = vec![eth::TransactionTrace::default()];
    assert!(project(&b).is_err());
}

#[test]
fn block_version_does_not_gate_valid_or_ambiguous_balance_records() {
    let mut b = block();
    b.transaction_traces = vec![tx(eth::Call {
        balance_changes: vec![change(4, 10, Some(100), Some(80))],
        ..Default::default()
    })];
    b.balance_changes = vec![change(4, 20, Some(80), Some(90))];
    let expected = project(&b).unwrap();
    assert_eq!(expected.balances.len(), 1);
    assert_eq!(expected.balances[0].amount, "90");
    for version in [3, 4, 5, 99] {
        b.ver = version;
        assert_eq!(project(&b).unwrap(), expected, "version {version}");
        b.balance_changes[0].ordinal = 10;
        assert!(project(&b).unwrap_err().to_string().contains("ambiguous"), "version {version}");
        b.balance_changes[0].ordinal = 20;
    }
}

#[test]
fn empty_block_emits_no_rows_and_native_rows_omit_the_contract_field() {
    assert!(project(&block()).unwrap().balances.is_empty());
    let events = balances_pb::Events {
        balances: vec![balances_pb::Balance {
            contract: None,
            address: vec![4; 20],
            amount: "7".into(),
        }],
    };
    let bytes = events.encode_to_vec();
    // Events.balances (tag 1, length), then Balance.address (tag 2) directly:
    // an absent contract writes no field-1 bytes, unlike an empty byte string.
    assert_eq!(&bytes[..4], &[0x0a, 25, 0x12, 20]);
    let decoded = balances_pb::Events::decode_from_slice(bytes.as_slice()).unwrap();
    assert_eq!(decoded.balances[0].contract, None);
    let erc20 = balances_pb::Balance {
        contract: Some(vec![]),
        ..events.balances[0].clone()
    };
    assert_eq!(erc20.encode_to_vec()[..2], [0x0a, 0]);
}

#[test]
fn records_expose_scope_and_reason_for_the_reason_matrix() {
    let b = eth::Block::decode_from_slice(FIXTURE_BLOCK).unwrap();
    let records = records(&b).unwrap();
    assert!(records.len() > 82);
    let system = hex::decode(SYSTEM_FEE_ADDRESS).unwrap();
    // The BSC v5 producer records the block-level reset under the ordinary
    // fee-reward reason; the fixture carries no REWARD_FEE_RESET record.
    assert!(records
        .iter()
        .any(|r| r.scope == persist::Scope::Block && r.address == system && r.reason == eth::balance_change::Reason::RewardTransactionFee as i32));
    assert!(records
        .iter()
        .any(|r| r.scope == persist::Scope::Tx && r.reason == eth::balance_change::Reason::Transfer as i32));
    let mut matrix: BTreeMap<(String, i32), u64> = BTreeMap::new();
    for r in &records {
        *matrix.entry((format!("{:?}", r.scope), r.reason)).or_default() += 1;
    }
    println!("{matrix:?}");
    assert!(records.iter().all(|r| r.address.len() == 20 && r.ordinal > 0));
}

#[test]
fn failed_transactions_with_unpinned_reasons_fail_closed() {
    let mut b = block();
    let failed = |reason: i32| eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Failed.into(),
        from: vec![8; 20],
        calls: vec![eth::Call {
            state_reverted: true,
            balance_changes: vec![
                eth::BalanceChange {
                    reason: eth::balance_change::Reason::GasBuy.into(),
                    ..change(8, 20, Some(100), Some(90))
                },
                eth::BalanceChange {
                    reason: reason.into(),
                    ..change(9, 21, Some(0), Some(50))
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    // Reverted transfer-like effects are pinned: dropped, gas kept.
    for reason in [
        eth::balance_change::Reason::Transfer,
        eth::balance_change::Reason::TouchAccount,
        eth::balance_change::Reason::Burn,
        eth::balance_change::Reason::SuicideWithdraw,
    ] {
        b.transaction_traces = vec![failed(reason as i32)];
        let rows = changes(&b).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].amount, "90");
    }
    // BNB blob-fee rewards (17), OP-stack mints (18), reverts (19), unknown
    // future reasons and block-level reasons inside a failed root call have no
    // fixture: the block is rejected rather than reduced without the record.
    for reason in [
        17,
        18,
        19,
        20,
        99,
        eth::balance_change::Reason::RewardFeeReset as i32,
        eth::balance_change::Reason::Withdrawal as i32,
    ] {
        b.transaction_traces = vec![failed(reason)];
        assert!(changes(&b).unwrap_err().to_string().contains("pinned persistence"), "reason {reason}");
    }
    // The same reasons in a successful transaction are ordinary persisted state.
    let mut ok = failed(17);
    ok.status = eth::TransactionTraceStatus::Succeeded.into();
    ok.calls[0].state_reverted = false;
    b.transaction_traces = vec![ok];
    assert_eq!(changes(&b).unwrap().len(), 2);
}

/// WETH9-style wrapping on captured WBNB transactions: the holder's WBNB is an
/// ERC-20 balance and the wrapper contract's BNB is native account state. The
/// wrapper's native balance moves by the event's amount; the holder gets no
/// second native balance for it.
mod wrapper_backing {
    use super::*;
    use substreams::scalar::BigInt;

    const WBNB: &str = "bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
    const DEPOSIT: &str = "e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c";
    const WITHDRAWAL: &str = "7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65";

    /// The holder and amount of the block's one persisted WBNB `topic0` log.
    fn wrapped(block: &eth::Block, topic0: &str) -> (Vec<u8>, BigInt) {
        let logs: Vec<_> = block
            .transaction_traces
            .iter()
            .filter(|tx| tx.status == eth::TransactionTraceStatus::Succeeded)
            .flat_map(|tx| tx.calls.iter().filter(|c| !c.state_reverted))
            .flat_map(|c| &c.logs)
            .filter(|l| hex::encode(&l.address) == WBNB && l.topics.first().is_some_and(|t| hex::encode(t) == topic0))
            .collect();
        assert_eq!(logs.len(), 1);
        (logs[0].topics[1][12..].to_vec(), BigInt::from_unsigned_bytes_be(&logs[0].data))
    }

    fn delta(old: &str, new: &str) -> BigInt {
        new.parse::<BigInt>().unwrap() - old.parse::<BigInt>().unwrap()
    }

    #[test]
    fn deposit_moves_native_bnb_into_the_wrapper() {
        let block = eth::Block::decode_from_slice(
            include_bytes!("../../../erc20/balances/tests/fixtures/wbnb-mutations/122288015-tx18-deposit-nested-no-transfer.pb").as_slice(),
        )
        .unwrap();
        let (holder, wad) = wrapped(&block, DEPOSIT);
        assert_eq!(wad, BigInt::from(100971252078042364u64));
        let native = changes(&block).unwrap();
        let wrapper = native.iter().find(|r| hex::encode(&r.address) == WBNB).unwrap();
        assert_eq!(delta(&wrapper.old_amount, &wrapper.amount), wad);
        // The holder's own native balance nets to zero (received, then
        // forwarded the BNB it wrapped).
        let holder = native.iter().find(|r| r.address == holder).unwrap();
        assert_eq!((holder.old_amount.as_str(), holder.records), (holder.amount.as_str(), 2));
    }

    #[test]
    fn withdrawal_releases_native_bnb_from_the_wrapper() {
        let block = eth::Block::decode_from_slice(
            include_bytes!("../../../erc20/balances/tests/fixtures/wbnb-mutations/122288035-tx9-withdrawal-no-transfer.pb").as_slice(),
        )
        .unwrap();
        let (_, wad) = wrapped(&block, WITHDRAWAL);
        assert_eq!(wad, BigInt::from(69434307925719935u64));
        let native = changes(&block).unwrap();
        let wrapper = native.iter().find(|r| hex::encode(&r.address) == WBNB).unwrap();
        assert_eq!(delta(&wrapper.old_amount, &wrapper.amount), -wad);
    }

    #[test]
    fn reverted_deposit_leaves_no_wrapper_native_row() {
        let block = eth::Block::decode_from_slice(
            include_bytes!("../../../erc20/balances/tests/fixtures/wbnb-mutations/122288021-tx35-reverted-transaction-deposit.pb").as_slice(),
        )
        .unwrap();
        let native = changes(&block).unwrap();
        assert!(!native.iter().any(|r| hex::encode(&r.address) == WBNB));
        // Only the sender's gas debit and the system fee credit persist.
        assert_eq!(native.len(), 2);
    }
}
