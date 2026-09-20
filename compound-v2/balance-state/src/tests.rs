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
    } = m.cash.clone()
    else {
        panic!()
    };
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
    let e = &events.epochs[0];
    assert_eq!(
        (e.family, e.basis_kind, &e.balance_asset, e.balance_decimals, e.kind),
        (
            pb::ModelFamily::CompoundV2Ctoken as i32,
            pb::BasisKind::Shares as i32,
            &m.ctoken,
            8,
            pb::EpochEventKind::Bound as i32
        )
    );
    let roles: Vec<i32> = events.dependencies.iter().filter(|d| d.market == m.ctoken).map(|d| d.role).collect();
    assert_eq!(roles, vec![pb::DependencyRole::Underlying as i32, pb::DependencyRole::InterestRateModel as i32]);
    assert_eq!(events.dependencies.iter().filter(|d| d.market == ceth().ctoken).count(), 1);
    let constants: Vec<(i32, String)> = events
        .global_state
        .iter()
        .filter(|g| g.observation == pb::Observation::QualifiedConstant as i32 && g.market == ceth().ctoken)
        .map(|g| (g.field, g.value.clone()))
        .collect();
    assert_eq!(
        constants,
        vec![
            (pb::StateField::CompoundV2IrmBaseRatePerBlock as i32, "0".into()),
            (pb::StateField::CompoundV2IrmMultiplierPerBlock as i32, "95129375951".into()),
            (pb::StateField::CompoundV2IrmBlocksPerYear as i32, "2102400".into()),
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
    for (address, reason) in [
        (m.ctoken.clone(), pb::InvalidationReason::CodeChange),
        (m.rate_model.clone(), pb::InvalidationReason::RateModelChange),
        (m.underlying.clone().unwrap(), pb::InvalidationReason::DependencyCodeChange),
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
    v["markets"][0]["implementation_slot"] = "0x0000000000000000000000000000000000000000000000000000000000000012".into();
    v["markets"][0]["implementation"] = "0x99ee778b9a6205657dd03b2b91415c8646d521ec".into();
    let delegator = parse(&v.to_string()).unwrap();
    b.transaction_traces = vec![tx(eth::Call {
        address: m.ctoken.clone(),
        storage_changes: vec![write(&m.ctoken, w(0x12), w(1), w(2), 10)],
        ..Default::default()
    })];
    let events = project(&b, &delegator).unwrap();
    assert_eq!(events.epochs[0].reason, pb::InvalidationReason::ImplementationPointerWrite as i32);
    let bound = project(&block(1), &delegator).unwrap();
    let implementation = bound.dependencies.iter().find(|d| d.role == pb::DependencyRole::Implementation as i32).unwrap();
    assert_eq!(
        (implementation.binding, &implementation.pointer_slot),
        (pb::BindingKind::StoragePointer as i32, &w(0x12).to_vec())
    );
}

#[test]
fn reviewed_storage_is_ignored_and_unknown_ctoken_writes_fail_closed() {
    let m = cusdc();
    let cfg = config();
    let mut b = block(10);
    // Reentrancy flag flip, and the second word of a BorrowSnapshot struct.
    let borrows_base = m.other_mapping_slots[1];
    let snapshot_key = mapping_key(&[4; 20], &borrows_base);
    let mut second = snapshot_key;
    second[31] = second[31].wrapping_add(1);
    if second[31] == 0 {
        second = w(0x1234); // avoid the carry edge in this synthetic case
    }
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
