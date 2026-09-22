use super::*;

const EPOCHS: &str = include_str!("../tests/fixtures/mainnet-ctoken-epochs.json");

fn config() -> Config {
    parse(EPOCHS).unwrap()
}
fn cusdc() -> Market {
    config().markets[0].clone()
}
fn ceth() -> Market {
    config().markets[1].clone()
}
fn block(number: u64) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp { seconds: 1789689600, nanos: 0 }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn tx(call: eth::Call) -> eth::TransactionTrace {
    eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 9,
        calls: vec![call],
        ..Default::default()
    }
}
fn w(v: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[16..].copy_from_slice(&v.to_be_bytes());
    out
}
fn write(address: &[u8], key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn preimage(holder: &[u8], base: &[u8; 32]) -> (String, String) {
    let mut p = vec![0u8; 64];
    p[12..32].copy_from_slice(holder);
    p[32..].copy_from_slice(base);
    (hex::encode(mapping_key(holder, base)), hex::encode(p))
}
fn shares_call(market: &Market, holder: &[u8], old: u128, new: u128, ordinal: u64) -> eth::Call {
    let key = mapping_key(holder, &market.account_tokens_slot);
    eth::Call {
        index: 1,
        address: market.ctoken.clone(),
        keccak_preimages: [preimage(holder, &market.account_tokens_slot)].into(),
        storage_changes: vec![write(&market.ctoken, key, w(old), w(new), ordinal)],
        ..Default::default()
    }
}
fn fields(events: &pb::Events) -> Vec<(i32, String, String, i32)> {
    events
        .global_state
        .iter()
        .map(|g| (g.field, g.previous_value.clone(), g.value.clone(), g.observation))
        .collect()
}

#[test]
fn share_writes_are_holder_rows_and_market_words_are_global_rows() {
    let m = cusdc();
    let cfg = config();
    let mut b = block(10);
    let mut call = shares_call(&m, &[9; 20], 1_000_000_000, 1_250_000_000, 10);
    let slot_of = |f: pb::StateField| m.scalars.iter().find(|(_, x, _)| *x == f).unwrap().0;
    call.storage_changes.extend([
        write(&m.ctoken, slot_of(pb::StateField::CompoundV2AccrualBlockNumber), w(9), w(10), 11),
        write(
            &m.ctoken,
            slot_of(pb::StateField::CompoundV2BorrowIndex),
            w(1_050_000_000_000_000_000),
            w(1_050_000_100_000_000_000),
            12,
        ),
        write(
            &m.ctoken,
            slot_of(pb::StateField::CompoundV2TotalBorrows),
            w(5_000_000_000),
            w(5_000_001_000),
            13,
        ),
        write(&m.ctoken, slot_of(pb::StateField::CompoundV2TotalReserves), w(70), w(71), 14),
        write(
            &m.ctoken,
            slot_of(pb::StateField::CompoundV2TotalSupply),
            w(2_000_000_000_000),
            w(2_000_250_000_000),
            15,
        ),
    ]);
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
    let h = &events.holder_basis[0];
    assert_eq!(
        (&*h.previous_value, &*h.value, h.basis_kind, h.bit_width, h.signed),
        ("1000000000", "1250000000", pb::BasisKind::Shares as i32, 256, false)
    );
    assert_eq!(
        fields(&events),
        vec![
            (pb::StateField::CompoundV2TotalBorrows as i32, "5000000000".into(), "5000001000".into(), 1),
            (pb::StateField::CompoundV2TotalReserves as i32, "70".into(), "71".into(), 1),
            (pb::StateField::CompoundV2TotalSupply as i32, "2000000000000".into(), "2000250000000".into(), 1),
            (
                pb::StateField::CompoundV2BorrowIndex as i32,
                "1050000000000000000".into(),
                "1050000100000000000".into(),
                1
            ),
            (pb::StateField::CompoundV2AccrualBlockNumber as i32, "9".into(), "10".into(), 1),
        ]
    );
    let index = events
        .global_state
        .iter()
        .find(|g| g.field == pb::StateField::CompoundV2BorrowIndex as i32)
        .unwrap();
    assert_eq!(index.scale, EXP_SCALE);
    assert!(events.epochs.is_empty());
    assert_eq!((events.clocks[0].holder_basis_count, events.clocks[0].global_state_count), (1, 5));
    // Same-block repeated writes reduce to first-old / last-new.
    let mut call = shares_call(&m, &[9; 20], 1, 2, 10);
    let key = mapping_key(&[9; 20], &m.account_tokens_slot);
    call.storage_changes.push(write(&m.ctoken, key, w(2), w(5), 11));
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (
            &*events.holder_basis[0].previous_value,
            &*events.holder_basis[0].value,
            events.holder_basis[0].change_count
        ),
        ("1", "5", 2)
    );
}

#[test]
fn erc20_cash_comes_only_from_the_qualified_underlying_mapping_entry() {
    let m = cusdc();
    let cfg = config();
    let underlying = m.underlying.clone().unwrap();
    let Cash::Erc20Mapping {
        balances_slot,
        implementation_slot,
        value_bits,
        ..
    } = m.cash.clone()
    else {
        panic!()
    };
    assert_eq!(value_bits, 255);
    let mut b = block(10);
    // A donation: USDC transfer to the cToken with no cToken write at all.
    let cash_key = mapping_key(&m.ctoken, &balances_slot);
    let other_key = mapping_key(&[5; 20], &balances_slot);
    b.transaction_traces = vec![tx(eth::Call {
        address: underlying.clone(),
        storage_changes: vec![
            write(&underlying, other_key, w(900), w(400), 10),
            write(&underlying, cash_key, w(1_000_000), w(1_000_500), 11),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events),
        vec![(pb::StateField::CompoundV2TotalCash as i32, "1000000".into(), "1000500".into(), 1)]
    );
    let cash = &events.global_state[0];
    assert_eq!((&cash.key, &cash.storage_contract, &cash.market), (&m.ctoken, &underlying, &m.ctoken));
    assert_eq!(events.global_state[0].bit_width, 255);
    // FiatToken V2.2 keeps the blacklist flag in bit 255 of the same word;
    // `_balanceOf` masks it, so the row carries only the low 255 bits.
    let mut blacklisted = w(1_000_500);
    blacklisted[0] |= 0x80;
    b.transaction_traces = vec![tx(eth::Call {
        address: underlying.clone(),
        storage_changes: vec![write(&underlying, cash_key, w(1_000_500), blacklisted, 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (&*events.global_state[0].previous_value, &*events.global_state[0].value),
        ("1000500", "1000500")
    );
    assert_eq!(events.global_state[0].raw_word[0], 0x80);
    // A write to the underlying's implementation pointer invalidates the epoch.
    b.transaction_traces = vec![tx(eth::Call {
        address: underlying.clone(),
        storage_changes: vec![write(&underlying, implementation_slot.unwrap(), w(1), w(2), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 1);
    assert_eq!(
        (events.epochs[0].kind, events.epochs[0].reason),
        (pb::EpochEventKind::Invalidated as i32, pb::InvalidationReason::DependencyPointerWrite as i32)
    );
    assert_eq!(events.epochs[0].evidence_contract, underlying);
}

#[test]
fn native_cash_comes_from_persisted_balance_changes_of_the_cether_contract() {
    let m = ceth();
    let cfg = config();
    let mut b = block(10);
    let balance = |old: u128, new: u128, ordinal: u64| eth::BalanceChange {
        address: m.ctoken.clone(),
        old_value: Some(eth::BigInt { bytes: w(old)[16..].to_vec() }),
        new_value: Some(eth::BigInt { bytes: w(new)[16..].to_vec() }),
        ordinal,
        reason: eth::balance_change::Reason::Transfer as i32,
    };
    // Two transfers in one block reduce; a reverted frame is ignored.
    b.transaction_traces = vec![
        tx(eth::Call {
            balance_changes: vec![balance(10, 15, 10), balance(15, 12, 11)],
            ..Default::default()
        }),
        eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            hash: vec![8; 32],
            index: 10,
            calls: vec![eth::Call {
                state_reverted: true,
                balance_changes: vec![balance(12, 99, 12)],
                ..Default::default()
            }],
            ..Default::default()
        },
    ];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(fields(&events), vec![(pb::StateField::CompoundV2TotalCash as i32, "10".into(), "12".into(), 1)]);
    assert_eq!((&events.global_state[0].storage_contract, events.global_state[0].change_count), (&m.ctoken, 2));
    // The cUSDC market ignores native balance changes of its cToken.
    let other = cusdc();
    b.transaction_traces = vec![tx(eth::Call {
        balance_changes: vec![eth::BalanceChange {
            address: other.ctoken.clone(),
            old_value: None,
            new_value: Some(eth::BigInt { bytes: vec![1] }),
            ordinal: 10,
            reason: 6,
        }],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap().global_state.is_empty());
}

#[test]
fn rate_model_storage_and_constants_are_carried_and_bound() {
    let m = cusdc();
    let cfg = config();
    let mut b = block(10);
    let (kink_slot, _) = m.rate_model_slots.iter().find(|(_, f)| *f == pb::StateField::CompoundV2IrmKink).unwrap();
    b.transaction_traces = vec![tx(eth::Call {
        address: m.rate_model.clone(),
        storage_changes: vec![
            write(&m.rate_model, *kink_slot, w(800_000_000_000_000_000), w(850_000_000_000_000_000), 10),
            write(&m.rate_model, w(0), w(1), w(2), 11), // owner: not an input
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events),
        vec![(
            pb::StateField::CompoundV2IrmKink as i32,
            "800000000000000000".into(),
            "850000000000000000".into(),
            1
        )]
    );
    assert_eq!(events.global_state[0].storage_contract, m.rate_model);
    // Activation binds the epoch, the dependencies and the constants.
    let events = project(&block(1), &cfg).unwrap();
    assert_eq!(events.epochs.len(), 2);
    // The evaluated balance is the underlying (USDC, 6 decimals); the basis is
    // cToken shares converted by a 1e18 exchange-rate mantissa.
    let e = &events.epochs[0];
    assert_eq!(
        (e.family, e.basis_kind, &e.balance_asset, e.balance_decimals, &*e.basis_scale, e.kind),
        (
            pb::ModelFamily::CompoundV2Ctoken as i32,
            pb::BasisKind::Shares as i32,
            m.underlying.as_ref().unwrap(),
            6,
            EXP_SCALE,
            pb::EpochEventKind::Bound as i32
        )
    );
    let e = events.epochs.iter().find(|e| e.market == ceth().ctoken).unwrap();
    assert_eq!((e.balance_asset.is_empty(), e.balance_decimals), (true, 18));
    // Sorted by (depth, role): the underlying and rate model are cToken
    // storage pointers; the USDC implementation hangs from the underlying.
    let Cash::Erc20Mapping {
        underlying_slot,
        implementation_slot: Some(usdc_slot),
        implementation: Some(usdc_implementation),
        ..
    } = m.cash.clone()
    else {
        panic!()
    };
    // (depth, role, binding, pointer contract, pointer slot, parent)
    type Edge = (u32, i32, i32, Vec<u8>, Vec<u8>, Vec<u8>);
    let edges: Vec<Edge> = events
        .dependencies
        .iter()
        .filter(|d| d.market == m.ctoken)
        .map(|d| (d.depth, d.role, d.binding, d.pointer_contract.clone(), d.pointer_slot.clone(), d.parent.clone()))
        .collect();
    let pointer = pb::BindingKind::StoragePointer as i32;
    let usdc = m.underlying.clone().unwrap();
    assert_eq!(
        edges,
        vec![
            (
                1,
                pb::DependencyRole::Underlying as i32,
                pointer,
                m.ctoken.clone(),
                underlying_slot.to_vec(),
                vec![]
            ),
            (
                1,
                pb::DependencyRole::InterestRateModel as i32,
                pointer,
                m.ctoken.clone(),
                m.rate_model_slot.to_vec(),
                vec![]
            ),
            (
                2,
                pb::DependencyRole::Implementation as i32,
                pointer,
                usdc.clone(),
                usdc_slot.to_vec(),
                usdc.clone()
            ),
        ]
    );
    assert_eq!(events.dependencies[2].pointer_value, word(&usdc_implementation).unwrap().to_vec());
    assert_eq!(events.dependencies.iter().filter(|d| d.market == ceth().ctoken).count(), 1);
    let constants: Vec<(i32, String)> = events
        .global_state
        .iter()
        .filter(|g| g.observation == pb::Observation::QualifiedConstant as i32 && g.market == ceth().ctoken)
        .map(|g| (g.field, g.value.clone()))
        .collect();
    // The 2019 WhitePaper model stores per-year parameters (no setter).
    assert_eq!(
        constants,
        vec![
            (pb::StateField::CompoundV2IrmBlocksPerYear as i32, "2102400".into()),
            (pb::StateField::CompoundV2IrmBaseRatePerYear as i32, "0".into()),
            (pb::StateField::CompoundV2IrmMultiplierPerYear as i32, "200000000000000000".into()),
        ]
    );
    assert_eq!(events.global_state.iter().filter(|g| g.market == m.ctoken).count(), 1);
    assert!(project(&block(2), &cfg).unwrap().epochs.is_empty());
    assert_eq!(project(&block(1001), &cfg).unwrap().epochs[0].kind, pb::EpochEventKind::Reaffirmed as i32);
}

#[test]
fn dependency_changes_invalidate_with_evidence_instead_of_failing() {
    let m = cusdc();
    let cfg = config();
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, m.rate_model_slot, w(1), w(2), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (events.epochs[0].kind, events.epochs[0].reason, &events.epochs[0].evidence_slot),
        (
            pb::EpochEventKind::Invalidated as i32,
            pb::InvalidationReason::RateModelChange as i32,
            &m.rate_model_slot.to_vec()
        )
    );
    assert_eq!(events.epochs[0].evidence_word, w(2).to_vec());
    // The cToken's `underlying` word is a pointer too.
    let Cash::Erc20Mapping {
        underlying_slot,
        implementation: Some(usdc_implementation),
        ..
    } = m.cash.clone()
    else {
        panic!()
    };
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, underlying_slot, w(1), w(2), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (events.epochs.len(), events.epochs[0].reason, &events.epochs[0].evidence_slot),
        (1, pb::InvalidationReason::DependencyPointerWrite as i32, &underlying_slot.to_vec())
    );
    for (address, reason) in [
        (m.ctoken.clone(), pb::InvalidationReason::CodeChange),
        (m.rate_model.clone(), pb::InvalidationReason::RateModelChange),
        (m.underlying.clone().unwrap(), pb::InvalidationReason::DependencyCodeChange),
        (usdc_implementation.clone(), pb::InvalidationReason::DependencyCodeChange),
    ] {
        b.transaction_traces = vec![tx(eth::Call {
            code_changes: vec![eth::CodeChange {
                address,
                old_hash: vec![1; 32],
                new_hash: vec![2; 32],
                ordinal: 5,
                ..Default::default()
            }],
            ..Default::default()
        })];
        let events = project(&b, &cfg).unwrap();
        assert_eq!(
            (events.epochs.len(), events.epochs[0].reason, &events.epochs[0].evidence_code_hash),
            (1, reason as i32, &vec![2; 32])
        );
    }
    // A delegator market invalidates on its implementation pointer.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["kind"] = "cerc20_delegator".into();
    v["markets"][0]["implementation_slot"] = "0x0000000000000000000000000000000000000000000000000000000000000013".into();
    v["markets"][0]["implementation"] = "0x99ee778b9a6205657dd03b2b91415c8646d521ec".into();
    let delegator = parse(&v.to_string()).unwrap();
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, w(0x13), w(1), w(2), 10)],
        ..Default::default()
    })];
    let events = project(&b, &delegator).unwrap();
    assert_eq!(events.epochs[0].reason, pb::InvalidationReason::ImplementationPointerWrite as i32);
    let bound = project(&block(1), &delegator).unwrap();
    let implementation = bound
        .dependencies
        .iter()
        .find(|d| d.role == pb::DependencyRole::Implementation as i32 && d.depth == 1)
        .unwrap();
    assert_eq!(
        (implementation.binding, &implementation.pointer_slot),
        (pb::BindingKind::StoragePointer as i32, &w(0x13).to_vec())
    );
    // An upgrade whose `_becomeImplementation` writes storage the old epoch
    // never described (CDaiDelegate.sol:52-54) ends the epoch at the pointer
    // write: the block carries the evidence instead of failing.
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![
            write(&m.ctoken, w(0x13), w(1), w(2), 10),
            write(&m.ctoken, w(0x14), w(0), w(3), 11),
            write(&m.ctoken, w(0x15), w(0), w(4), 12),
            write(&m.ctoken, m.scalars[3].0, w(5), w(6), 13),
        ],
        ..Default::default()
    })];
    let events = project(&b, &delegator).unwrap();
    assert_eq!((events.epochs.len(), events.epochs[0].ordinal, events.global_state.len()), (1, 10, 0));
    // Before the invalidation the same unknown write still fails closed.
    b.transaction_traces[0].calls[0].storage_changes[1].ordinal = 9;
    assert!(project(&b, &delegator).unwrap_err().to_string().contains("unresolved"));
}

#[test]
fn reviewed_storage_is_ignored_and_unknown_ctoken_writes_fail_closed() {
    let m = cusdc();
    let cfg = config();
    let mut b = block(10);
    // Reentrancy flag flip, and the second word of a BorrowSnapshot struct.
    let borrows_base = m.other_mapping_slots[1];
    let snapshot_key = mapping_key(&[4; 20], &borrows_base);
    // Word 1 of the BorrowSnapshot struct, with carry.
    let second = add_small(&snapshot_key, 1);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        keccak_preimages: [preimage(&[4; 20], &borrows_base)].into(),
        storage_changes: vec![
            write(&m.ctoken, w(0), w(1), w(0), 10),
            write(&m.ctoken, w(0), w(0), w(1), 11),
            write(&m.ctoken, snapshot_key, w(0), w(5), 12),
            write(&m.ctoken, second, w(0), w(6), 13),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty() && events.epochs.is_empty());
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, w(0x77), w(0), w(1), 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("unresolved"));
    // A share write in a reverted frame or a failed transaction is nothing.
    let mut call = shares_call(&m, &[9; 20], 1, 2, 10);
    call.state_reverted = true;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    let mut failed = tx(shares_call(&m, &[9; 20], 1, 2, 10));
    failed.status = eth::TransactionTraceStatus::Failed as i32;
    b.transaction_traces = vec![failed];
    assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    // Ties and discontinuities.
    let mut call = shares_call(&m, &[9; 20], 1, 2, 10);
    let key = mapping_key(&[9; 20], &m.account_tokens_slot);
    call.storage_changes.push(write(&m.ctoken, key, w(3), w(4), 11));
    b.transaction_traces = vec![tx(call.clone())];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("discontinuous"));
    call.storage_changes[1] = write(&m.ctoken, key, w(2), w(4), 10);
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("ambiguous"));
    let mut b = block(10);
    b.ver = 4;
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("producer version"));
}

#[test]
fn parameters_are_explicit_and_fail_closed() {
    assert!(parse(r#"{"chain_id":1,"producer_versions":[5],"markets":[]}"#).unwrap().markets.is_empty());
    let mutate = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
        f(&mut v);
        parse(&v.to_string()).map(|_| ()).unwrap_err().to_string()
    };
    assert!(mutate(&|v| v["markets"][0]["underlying"]["cash"] = "native".into()).contains("cash kind"));
    assert!(mutate(&|v| v["markets"][0]["underlying"]["source_pin"] = "".into()).contains("qualified underlying"));
    assert!(mutate(&|v| v["markets"][0]["underlying"]["balances_slot"] = serde_json::Value::Null).contains("balances_slot"));
    assert!(mutate(&|v| v["markets"][0]["underlying"]["value_bits"] = 0.into()).contains("value_bits"));
    assert!(mutate(&|v| v["markets"][0]["underlying"]["value_bits"] = 257.into()).contains("value_bits"));
    assert!(mutate(&|v| v["markets"][1]["underlying"]["value_bits"] = 255.into()).contains("native cash"));
    // Without value_bits the full word is the balance.
    let mut full: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    full["markets"][0]["underlying"].as_object_mut().unwrap().remove("value_bits");
    let Cash::Erc20Mapping { value_bits, .. } = parse(&full.to_string()).unwrap().markets[0].cash.clone() else {
        panic!()
    };
    assert_eq!(value_bits, 256);
    assert!(mutate(&|v| v["markets"][1]["underlying"]["address"] = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into()).contains("native cash"));
    assert!(mutate(&|v| v["markets"][0]["slots"]["total_supply"] = v["markets"][0]["slots"]["borrow_index"].clone()).contains("overlap"));
    assert!(mutate(&|v| v["markets"][0]["rate_model"]["constants"]["kink"] = "1".into()).contains("both slot and constant"));
    assert!(mutate(&|v| v["markets"][0]["rate_model"]["constants"] = serde_json::json!({})).contains("blocks_per_year"));
    assert!(mutate(&|v| v["markets"][0]["rate_model"]["slots"]["owner"] = "0x00".into()).contains("unknown rate model parameter"));
    assert!(mutate(&|v| v["markets"][0]["kind"] = "cerc20_delegator".into()).contains("implementation_slot"));
    assert!(mutate(&|v| v["markets"][0]["implementation"] = "0x99ee778b9a6205657dd03b2b91415c8646d521ec".into()).contains("no implementation"));
    assert!(mutate(&|v| v["markets"][1]["ctoken"] = v["markets"][0]["ctoken"].clone()).contains("duplicate"));
    assert!(mutate(&|v| v["markets"][0]["extra"] = 1.into()).contains("unknown field"));
}

#[test]
fn mapping_member_walks_struct_words_and_nested_mappings() {
    let base = w(16);
    let owner = mapping_key(&[1; 20], &base);
    let nested = mapping_key(&[2; 20], &owner);
    let mut preimages = BTreeMap::new();
    for (k, v) in [preimage(&[1; 20], &base), preimage(&[2; 20], &owner)] {
        preimages.insert(word(&hex::decode(k).unwrap()).unwrap(), hex::decode(v).unwrap());
    }
    assert!(mapping_member(owner, &preimages, &base));
    assert!(mapping_member(nested, &preimages, &base));
    let mut plus_one = owner;
    let bumped = sub_small(&plus_one, 0).unwrap();
    assert_eq!(bumped, owner);
    // key + 1 resolves; key + MAX_STRUCT_WORDS does not.
    plus_one = add_small(&owner, 1);
    assert!(mapping_member(plus_one, &preimages, &base));
    assert!(!mapping_member(add_small(&owner, MAX_STRUCT_WORDS), &preimages, &base));
    assert!(!mapping_member(owner, &preimages, &w(17)));
    assert_eq!(sub_small(&w(0), 1), None);
    assert_eq!(sub_small(&w(256), 1), Some(w(255)));
}
fn add_small(word: &[u8; 32], n: u8) -> [u8; 32] {
    let mut out = *word;
    let mut carry = n as u16;
    for byte in out.iter_mut().rev() {
        if carry == 0 {
            break;
        }
        let v = *byte as u16 + carry;
        *byte = (v & 0xff) as u8;
        carry = v >> 8;
    }
    out
}

#[test]
fn shared_hardening_rules_hold_for_ctokens() {
    use prost::Message;
    let cfg = config();
    let m = cusdc();
    // Only Extended producer versions 4 and 5 are qualified.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["producer_versions"] = serde_json::json!([3]);
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("4 and 5"));
    v["producer_versions"] = serde_json::json!([4, 5]);
    assert!(parse(&v.to_string()).is_ok());
    // blocksPerYear is a pinned constant of both rate models.
    v = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["rate_model"]["constants"]["blocks_per_year"] = "2628000".into();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("2102400"));
    // Carryover: shares persist across upgrades, rate-model rows do not.
    let bound = project(&block(1), &cfg).unwrap();
    assert!(bound.epochs.iter().all(|e| e.basis_carryover && !e.global_carryover));
    // A delegatecall frame (Call.address == implementation) writing the cToken's storage.
    let holder = [9u8; 20];
    let key = mapping_key(&holder, &m.account_tokens_slot);
    let mut b = block(10);
    b.transaction_traces = vec![eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 9,
        calls: vec![
            eth::Call {
                index: 0,
                address: m.ctoken.clone(),
                ..Default::default()
            },
            eth::Call {
                index: 1,
                parent_index: 0,
                depth: 1,
                call_type: eth::CallType::Delegate as i32,
                address: vec![0xee; 20],
                keccak_preimages: [preimage(&holder, &m.account_tokens_slot)].into(),
                storage_changes: vec![
                    write(&m.ctoken, key, w(1), w(2), 10),
                    write(&m.ctoken, w(0), w(1), w(0), 11),
                    write(&m.ctoken, w(0), w(0), w(1), 12),
                ],
                ..Default::default()
            },
        ],
        ..Default::default()
    }];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((events.holder_basis.len(), &*events.holder_basis[0].value), (1, "2"));
    // FAILED and REVERTED transactions contribute nothing; status 0 is refused.
    for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
        let mut t = tx(shares_call(&m, &holder, 1, 2, 10));
        t.status = status as i32;
        b.transaction_traces = vec![t];
        assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    }
    let mut t = tx(eth::Call::default());
    t.status = 0;
    b.transaction_traces = vec![t];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("incomplete transaction"));
    // Blocks before activation emit only the clock, even for unresolved writes.
    v = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["activation_block"] = 100.into();
    v["markets"][1]["activation_block"] = 100.into();
    let late = parse(&v.to_string()).unwrap();
    let mut b = block(50);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, w(0x77), w(0), w(1), 10)],
        ..Default::default()
    })];
    let events = project(&b, &late).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty() && events.epochs.is_empty());
    assert_eq!(events.clocks.len(), 1);
    // Ordering faults name the contract, key and ordinals.
    let mut call = shares_call(&m, &holder, 1, 2, 10);
    call.storage_changes.push(write(&m.ctoken, key, w(3), w(4), 11));
    let mut b = block(10);
    b.transaction_traces = vec![tx(call)];
    let err = project(&b, &cfg).unwrap_err().to_string();
    assert!(err.contains("discontinuous") && err.contains(&hex::encode(&m.ctoken)) && err.contains(&hex::encode(key)) && err.contains("ordinal 11"));
    // Output is deterministic under input permutation.
    let mut b = block(10);
    let mut second = tx(shares_call(&m, &[8; 20], 5, 6, 12));
    second.index = 10;
    second.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&m, &holder, 1, 2, 10)), second];
    let forward = project(&b, &cfg).unwrap();
    let mut reversed = b.clone();
    reversed.transaction_traces.reverse();
    assert_eq!(forward.encode_to_vec(), project(&reversed, &cfg).unwrap().encode_to_vec());
}

#[test]
fn validate_block_refusals_provenance_and_multi_market_attribution() {
    let cfg = config();
    let usdc = cusdc();
    let eth = ceth();
    type Mutation = Box<dyn Fn(&mut eth::Block)>;
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "Extended blocks required",
            Box::new(|b| b.detail_level = eth::block::DetailLevel::DetaillevelBase as i32),
        ),
        ("producer version", Box::new(|b| b.ver = 3)),
        ("missing header", Box::new(|b| b.header = None)),
        ("invalid block identity", Box::new(|b| b.hash = vec![1; 31])),
        ("invalid block identity", Box::new(|b| b.header.as_mut().unwrap().state_root = vec![])),
        ("header number mismatch", Box::new(|b| b.header.as_mut().unwrap().number += 1)),
        ("missing timestamp", Box::new(|b| b.header.as_mut().unwrap().timestamp = None)),
        (
            "negative timestamp",
            Box::new(|b| b.header.as_mut().unwrap().timestamp.as_mut().unwrap().seconds = -1),
        ),
    ];
    for (message, apply) in cases {
        let mut b = block(10);
        apply(&mut b);
        let err = project(&b, &cfg).unwrap_err().to_string();
        assert!(err.contains(message), "expected `{message}`, got `{err}`");
    }
    // Same-block repeated writes keep the first old value and the last write's provenance.
    let holder = [9u8; 20];
    let mut b = block(10);
    let mut second = tx(shares_call(&usdc, &holder, 2, 7, 20));
    second.index = 10;
    second.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&usdc, &holder, 1, 2, 10)), second];
    let events = project(&b, &cfg).unwrap();
    let h = &events.holder_basis[0];
    assert_eq!(
        (
            &*h.previous_value,
            &*h.value,
            h.change_count,
            h.first_ordinal,
            h.ordinal,
            h.transaction_index,
            &h.transaction_hash
        ),
        ("1", "7", 2, 10, 20, 10, &vec![8; 32])
    );
    // Two markets written in one block are attributed by storage address.
    let mut b = block(10);
    let mut eth_tx = tx(shares_call(&eth, &holder, 5, 6, 12));
    eth_tx.index = 10;
    eth_tx.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&usdc, &holder, 1, 2, 10)), eth_tx];
    let events = project(&b, &cfg).unwrap();
    let rows: Vec<(&Vec<u8>, &str)> = events.holder_basis.iter().map(|h| (&h.market, h.value.as_str())).collect();
    let mut expected = vec![(&usdc.ctoken, "2"), (&eth.ctoken, "6")];
    expected.sort();
    assert_eq!(rows, expected);
    assert_eq!(events.clocks[0].holder_basis_count, 2);
}

#[test]
fn every_pointer_write_is_evidenced_including_excursions_and_equal_value_writes() {
    let cfg = config();
    let m = cusdc();
    let bound_irm = word(&m.rate_model).unwrap();
    let rogue = w(0xbad);
    // A rate-model excursion X -> Z -> X within one block: two invalidations
    // carrying the intermediate model, not one reduced X -> X.
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![
            write(&m.ctoken, m.rate_model_slot, bound_irm, rogue, 10),
            write(&m.ctoken, m.rate_model_slot, rogue, bound_irm, 12),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    let evidence: Vec<(i32, u64, Vec<u8>)> = events
        .epochs
        .iter()
        .filter(|e| e.market == m.ctoken)
        .map(|e| (e.reason, e.ordinal, e.evidence_word.clone()))
        .collect();
    assert_eq!(
        evidence,
        vec![
            (pb::InvalidationReason::RateModelChange as i32, 10, rogue.to_vec()),
            (pb::InvalidationReason::RateModelChange as i32, 12, bound_irm.to_vec()),
        ]
    );
    // An equal-value write to the underlying's proxy implementation slot is
    // dropped as a balance effect but still invalidates the dependency.
    let underlying = m.underlying.clone().unwrap();
    let Cash::Erc20Mapping {
        implementation_slot: Some(underlying_slot),
        ..
    } = m.cash.clone()
    else {
        panic!()
    };
    b.transaction_traces = vec![tx(eth::Call {
        address: underlying.clone(),
        storage_changes: vec![write(&underlying, underlying_slot, w(7), w(7), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 1);
    assert_eq!(
        (events.epochs[0].reason, &events.epochs[0].evidence_contract),
        (pb::InvalidationReason::DependencyPointerWrite as i32, &underlying)
    );
    // The same equal-value write on a non-pointer slot is not an effect at all.
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, m.scalars[0].0, w(7), w(7), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert!(events.epochs.is_empty() && events.global_state.is_empty());
    // The rate model is bound as a storage pointer on the cToken.
    let bound = project(&block(1), &cfg).unwrap();
    let irm = bound
        .dependencies
        .iter()
        .find(|d| d.market == m.ctoken && d.role == pb::DependencyRole::InterestRateModel as i32)
        .unwrap();
    assert_eq!(
        (irm.binding, &irm.pointer_contract, &irm.pointer_slot, &irm.pointer_value),
        (
            pb::BindingKind::StoragePointer as i32,
            &m.ctoken,
            &m.rate_model_slot.to_vec(),
            &bound_irm.to_vec()
        )
    );
}

#[test]
fn an_epoch_bound_mid_block_owns_only_effects_from_its_activation_ordinal() {
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["activation_block"] = 10.into();
    v["markets"][0]["activation_ordinal"] = 100.into();
    let cfg = parse(&v.to_string()).unwrap();
    let m = cfg.markets[0].clone();
    let holder = [9u8; 20];
    let key = mapping_key(&holder, &m.account_tokens_slot);
    let mut b = block(10);
    // The rate-model switch that starts this epoch (50) and a share write
    // under the previous epoch (60) are not this epoch's; the write at 150 is.
    let mut call = shares_call(&m, &holder, 1, 2, 60);
    call.storage_changes
        .insert(0, write(&m.ctoken, m.rate_model_slot, w(0xdead), word(&m.rate_model).unwrap(), 50));
    call.storage_changes.push(write(&m.ctoken, key, w(2), w(9), 150));
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    let mine: Vec<_> = events.epochs.iter().filter(|e| e.market == m.ctoken).collect();
    assert!(mine.iter().all(|e| e.kind != pb::EpochEventKind::Invalidated as i32));
    let bound_row = mine.iter().find(|e| e.kind == pb::EpochEventKind::Bound as i32).unwrap();
    assert_eq!((bound_row.activation_ordinal, bound_row.ordinal), (100, 100));
    let h: Vec<_> = events.holder_basis.iter().filter(|h| h.market == m.ctoken).collect();
    assert_eq!(h.len(), 1);
    assert_eq!((&*h[0].previous_value, &*h[0].value, h[0].first_ordinal), ("2", "9", 150));
    // An unresolved write before the activation ordinal belongs to the
    // previous epoch and does not refuse the block; after it, it does.
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, w(0x77), w(0), w(1), 60)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).is_ok());
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, w(0x77), w(0), w(1), 160)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("unresolved"));
}

fn native(address: &[u8], old: u128, new: u128, ordinal: u64) -> eth::BalanceChange {
    eth::BalanceChange {
        address: address.to_vec(),
        old_value: Some(eth::BigInt { bytes: w(old)[16..].to_vec() }),
        new_value: Some(eth::BigInt { bytes: w(new)[16..].to_vec() }),
        ordinal,
        reason: eth::balance_change::Reason::Transfer as i32,
    }
}

#[test]
fn accrual_moves_market_words_but_no_share_count() {
    let m = cusdc();
    let mut b = block(10);
    let slot = |field: pb::StateField| m.scalars.iter().find(|(_, f, _)| *f == field).unwrap().0;
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![
            // 2019 `nonReentrant` increments the guard counter and never restores it.
            write(&m.ctoken, w(0), w(41), w(42), 9),
            write(&m.ctoken, slot(pb::StateField::CompoundV2AccrualBlockNumber), w(9), w(10), 10),
            write(&m.ctoken, slot(pb::StateField::CompoundV2BorrowIndex), w(1_100), w(1_101), 11),
            write(&m.ctoken, slot(pb::StateField::CompoundV2TotalBorrows), w(500), w(501), 12),
            write(&m.ctoken, slot(pb::StateField::CompoundV2TotalReserves), w(7), w(8), 13),
        ],
        ..Default::default()
    })];
    let events = project(&b, &config()).unwrap();
    assert!(events.holder_basis.is_empty() && events.epochs.is_empty());
    assert_eq!(events.global_state.len(), 4);
}

#[test]
fn native_cash_names_no_storage_slot_and_follows_system_and_block_scopes() {
    let m = ceth();
    let cfg = config();
    // A system call and a block-level balance change (reward, withdrawal) are
    // persisted cash changes with their own scope and no transaction hash.
    let mut b = block(10);
    b.system_calls = vec![eth::Call {
        balance_changes: vec![native(&m.ctoken, 10, 15, 3)],
        ..Default::default()
    }];
    let events = project(&b, &cfg).unwrap();
    let row = &events.global_state[0];
    assert_eq!(
        (row.scope, row.transaction_hash.is_empty(), row.storage_slot.is_empty(), &row.storage_contract),
        (pb::Scope::SystemCall as i32, true, true, &m.ctoken)
    );
    let mut b = block(10);
    b.balance_changes = vec![native(&m.ctoken, 10, 20, 900)];
    let events = project(&b, &cfg).unwrap();
    let row = &events.global_state[0];
    assert_eq!((row.scope, &*row.value, row.storage_slot.is_empty()), (pb::Scope::Block as i32, "20", true));
    // Under a mid-block activation, cash moved before the ordinal is not this epoch's.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][1]["activation_block"] = 10.into();
    v["markets"][1]["activation_ordinal"] = 100.into();
    let late = parse(&v.to_string()).unwrap();
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        balance_changes: vec![native(&m.ctoken, 10, 15, 50), native(&m.ctoken, 15, 18, 150)],
        ..Default::default()
    })];
    let observed: Vec<(String, String)> = project(&b, &late)
        .unwrap()
        .global_state
        .iter()
        .filter(|g| g.observation == pb::Observation::ObservedWrite as i32)
        .map(|g| (g.previous_value.clone(), g.value.clone()))
        .collect();
    assert_eq!(observed, vec![("15".into(), "18".into())]);
}

#[test]
fn a_share_write_without_its_preimage_and_a_delegate_code_change_are_handled() {
    let m = cusdc();
    let cfg = config();
    let mut call = shares_call(&m, &[9; 20], 1, 2, 10);
    call.keccak_preimages.clear();
    let mut b = block(10);
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("unresolved"));
    // A delegator's implementation code change is the market's own code change.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["kind"] = "cerc20_delegator".into();
    v["markets"][0]["implementation_slot"] = "0x0000000000000000000000000000000000000000000000000000000000000013".into();
    v["markets"][0]["implementation"] = "0x99ee778b9a6205657dd03b2b91415c8646d521ec".into();
    let delegator = parse(&v.to_string()).unwrap();
    b.transaction_traces = vec![tx(eth::Call {
        code_changes: vec![eth::CodeChange {
            address: hex::decode("99ee778b9a6205657dd03b2b91415c8646d521ec").unwrap(),
            old_hash: vec![1; 32],
            new_hash: vec![2; 32],
            ordinal: 5,
            ..Default::default()
        }],
        ..Default::default()
    })];
    let events = project(&b, &delegator).unwrap();
    assert_eq!((events.epochs.len(), events.epochs[0].reason), (1, pb::InvalidationReason::CodeChange as i32));
}

#[test]
fn declarations_sit_at_the_activation_ordinal_and_parameters_are_canonical_decimals() {
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][1]["activation_block"] = 10.into();
    v["markets"][1]["activation_ordinal"] = 100.into();
    let cfg = parse(&v.to_string()).unwrap();
    let events = project(&block(10), &cfg).unwrap();
    let bound = events.epochs.iter().find(|e| e.market == ceth().ctoken).unwrap();
    assert_eq!(bound.ordinal, 100);
    let constants: Vec<u64> = events
        .global_state
        .iter()
        .filter(|g| g.market == ceth().ctoken && g.boundary == pb::Boundary::Declaration as i32)
        .map(|g| g.ordinal)
        .collect();
    assert_eq!(constants, vec![100, 100, 100]);
    let heartbeat = project(&block(1_010), &cfg).unwrap();
    assert!(heartbeat.global_state.iter().filter(|g| g.market == ceth().ctoken).all(|g| g.ordinal == 0));
    for bad in ["007", "", "1e18", &"9".repeat(79)] {
        let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
        v["markets"][1]["rate_model"]["constants"]["multiplier_per_year"] = bad.into();
        let error = parse(&v.to_string()).unwrap_err().to_string();
        assert!(error.contains("canonical uint256"), "{bad}: {error}");
    }
    // ERC-20 cash requires the cToken's underlying pointer; native cash refuses one.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["slots"].as_object_mut().unwrap().remove("underlying");
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("slots.underlying"));
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][1]["slots"]["underlying"] = "0x0000000000000000000000000000000000000000000000000000000000000012".into();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("native cash"));
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["underlying"].as_object_mut().unwrap().remove("implementation");
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("go together"));
}
