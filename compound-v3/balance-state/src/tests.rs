use super::*;
use prost::Message;

const EPOCH: &str = include_str!("../tests/fixtures/mainnet-cusdcv3-epoch.json");
const COMET: &str = "c3d688b66703497daa19211eedff47f25384cdc3";
const CWETH: &str = "a17581a9e3356d9a858b789d68b4d866e593ae94";
const WETH: &str = "c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
/// `keccak256("comet.reentrancy.guard")`, CometCore.sol:60 at the pinned commit.
const GUARD: &str = "c98c7730ba19013824f711a9ab74801459b27e6ff7685cb924587c89aeda53ac";

fn config() -> Config {
    parse(EPOCH).unwrap()
}
fn market() -> Market {
    config().markets[0].clone()
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
fn write_at(address: &[u8], key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn write(key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    write_at(&hex::decode(COMET).unwrap(), key, old, new, ordinal)
}
/// Pack little fields into a word: (offset, width, value) with value as u128.
fn pack(fields: &[(u32, u32, u128)]) -> [u8; 32] {
    let mut w = [0u8; 32];
    for &(offset, width, value) in fields {
        for bit in 0..width {
            if (value >> bit) & 1 == 1 {
                let target = offset + bit;
                w[31 - (target / 8) as usize] |= 1 << (target % 8);
            }
        }
    }
    w
}
fn w(v: u128) -> [u8; 32] {
    pack(&[(0, 128, v)])
}
fn preimage(holder: &[u8], base: &[u8; 32]) -> (String, String) {
    let mut p = vec![0u8; 64];
    p[12..32].copy_from_slice(holder);
    p[32..].copy_from_slice(base);
    (hex::encode(mapping_key(holder, base)), hex::encode(p))
}
fn user_basic_call(holder: &[u8], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::Call {
    let m = market();
    let key = mapping_key(holder, &m.user_basic_slot);
    eth::Call {
        index: 1,
        address: m.comet.clone(),
        keccak_preimages: [preimage(holder, &m.user_basic_slot)].into(),
        storage_changes: vec![write(key, old, new, ordinal)],
        ..Default::default()
    }
}
/// int104 two's complement inside the low 104 bits.
fn principal_word(principal: i128, tracking_index: u128) -> [u8; 32] {
    let unsigned = if principal < 0 { (1u128 << 104) as i128 + principal } else { principal } as u128;
    pack(&[(0, 104, unsigned), (104, 64, tracking_index)])
}
fn fields(events: &pb::Events) -> Vec<(i32, String, String)> {
    events
        .global_state
        .iter()
        .map(|g| (g.field, g.previous_value.clone(), g.value.clone()))
        .collect()
}
fn mutate(f: &dyn Fn(&mut serde_json::Value)) -> Result<Config, Error> {
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    f(&mut v);
    parse(&v.to_string())
}

#[test]
fn the_reviewed_guard_slot_is_the_pinned_label_and_scales_match_the_pinned_constants() {
    let m = market();
    assert_eq!(hex::encode(keccak(b"comet.reentrancy.guard")), GUARD);
    assert!(m.other_slots.contains(&keccak(b"comet.reentrancy.guard")));
    assert_eq!(BASE_INDEX_SCALE, "1000000000000000");
    assert_eq!(FACTOR_SCALE, "1000000000000000000");
    assert_eq!(
        hex::encode(m.implementation_slot),
        "360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
    );
}

#[test]
fn every_written_holder_word_is_a_row_with_the_signed_principal() {
    let mut b = block(10);
    let cfg = config();
    // Sign crossing: supplier becomes borrower.
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(5_000_000, 77), principal_word(-2_500_000, 78), 10))];
    let events = project(&b, &cfg).unwrap();
    let h = &events.holder_basis[0];
    assert_eq!((&*h.previous_value, &*h.value, h.signed, h.bit_width), ("5000000", "-2500000", true, 104));
    assert_eq!(h.basis_kind, pb::BasisKind::SignedPrincipal as i32);
    // A write that only moves tracking fields still yields a row (the word was written).
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(5, 1), principal_word(5, 2), 10))];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value), ("5", "5"));
    // Full withdrawal is a known zero; first borrow from zero keeps the previous zero.
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(5_000_000, 0), principal_word(0, 0), 10))];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value), ("5000000", "0"));
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(0, 0), principal_word(-100_000_000, 0), 10))];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value), ("0", "-100000000"));
    // A first-time holder whose old word Firehose encodes as empty bytes.
    let mut call = user_basic_call(&[9; 20], principal_word(0, 0), principal_word(1_000, 0), 10);
    call.storage_changes[0].old_value = vec![];
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value), ("0", "1000"));
    assert_eq!(events.clocks[0].holder_basis_count, 1);
    // int104 extremes round-trip.
    let min = -(1i128 << 103);
    let max = (1i128 << 103) - 1;
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(max, 0), principal_word(min, 0), 10))];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value),
        ("10141204801825835211973625643007", "-10141204801825835211973625643008")
    );
}

#[test]
fn same_block_repeated_writes_reduce_with_the_last_writes_provenance() {
    let cfg = config();
    let mut b = block(10);
    let first = tx(user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10));
    let mut second = tx(user_basic_call(&[9; 20], principal_word(2, 0), principal_word(-3, 0), 20));
    second.hash = vec![8; 32];
    second.index = 10;
    b.transaction_traces = vec![first, second];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
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
        ("1", "-3", 2, 10, 20, 10, &vec![8; 32])
    );
    assert_eq!(
        (&h.raw_previous_word, &h.raw_word),
        (&principal_word(1, 0).to_vec(), &principal_word(-3, 0).to_vec())
    );
}

#[test]
fn market_words_yield_one_row_per_decoded_field() {
    let m = market();
    let mut b = block(10);
    let old_indices = pack(&[(0, 64, 1_058_123_456_789_012), (64, 64, 1_074_987_654_321_098), (128, 64, 5), (192, 64, 6)]);
    let new_indices = pack(&[(0, 64, 1_058_200_000_000_000), (64, 64, 1_075_100_000_000_000), (128, 64, 5), (192, 64, 6)]);
    let old_totals = pack(&[
        (0, 104, 1_234_567_890_123_456),
        (104, 104, 987_654_321_098_765),
        (208, 40, 1_789_689_000),
        (248, 8, 0),
    ]);
    let new_totals = pack(&[
        (0, 104, 1_234_567_890_123_456),
        (104, 104, 990_000_000_000_000),
        (208, 40, 1_789_689_600),
        (248, 8, 0),
    ]);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![
            write(m.indices_slot, old_indices, new_indices, 10),
            write(m.totals_slot, old_totals, new_totals, 11),
        ],
        ..Default::default()
    })];
    let events = project(&b, &config()).unwrap();
    assert_eq!(
        fields(&events),
        vec![
            (
                pb::StateField::CometBaseSupplyIndex as i32,
                "1058123456789012".into(),
                "1058200000000000".into()
            ),
            (
                pb::StateField::CometBaseBorrowIndex as i32,
                "1074987654321098".into(),
                "1075100000000000".into()
            ),
            (
                pb::StateField::CometTotalSupplyBase as i32,
                "1234567890123456".into(),
                "1234567890123456".into()
            ),
            (pb::StateField::CometTotalBorrowBase as i32, "987654321098765".into(), "990000000000000".into()),
            (pb::StateField::CometLastAccrualTime as i32, "1789689000".into(), "1789689600".into()),
            (pb::StateField::CometPauseFlags as i32, "0".into(), "0".into()),
        ]
    );
    assert!(events.holder_basis.is_empty());
    assert_eq!((&*events.global_state[0].scale, events.global_state[0].bit_width), (BASE_INDEX_SCALE, 64));
    assert_eq!((events.global_state[4].bit_offset, events.global_state[4].bit_width), (208, 40));
    assert_eq!(events.clocks[0].global_state_count, 6);
    // A pause-flag-only write still states all four fields of the totals word.
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![write(
            m.totals_slot,
            old_totals,
            pack(&[
                (0, 104, 1_234_567_890_123_456),
                (104, 104, 987_654_321_098_765),
                (208, 40, 1_789_689_000),
                (248, 8, 0x1f),
            ]),
            10,
        )],
        ..Default::default()
    })];
    let events = project(&b, &config()).unwrap();
    assert_eq!(events.global_state.len(), 4);
    assert_eq!((&*events.global_state[3].previous_value, &*events.global_state[3].value), ("0", "31"));
}

#[test]
fn a_routine_supply_in_a_delegatecall_frame_with_the_reentrancy_guard_is_accepted() {
    let m = market();
    let cfg = config();
    let mut b = block(10);
    let holder = [9u8; 20];
    let key = mapping_key(&holder, &m.user_basic_slot);
    let guard = keccak(b"comet.reentrancy.guard");
    // The proxy frame has no writes; the implementation frame (Call.address ==
    // implementation) writes the Comet's storage and holds the preimage.
    let proxy = eth::Call {
        index: 0,
        address: m.comet.clone(),
        ..Default::default()
    };
    let implementation = eth::Call {
        index: 1,
        parent_index: 0,
        depth: 1,
        call_type: eth::CallType::Delegate as i32,
        address: m.implementation.clone(),
        keccak_preimages: [preimage(&holder, &m.user_basic_slot)].into(),
        storage_changes: vec![
            write(guard, w(0), w(1), 10),
            write(key, principal_word(1_000_000, 0), principal_word(6_000_000, 0), 11),
            write(m.totals_slot, w(0), pack(&[(0, 104, 5_000_000)]), 12),
            write(guard, w(1), w(0), 13),
        ],
        ..Default::default()
    };
    b.transaction_traces = vec![eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 9,
        calls: vec![proxy, implementation],
        ..Default::default()
    }];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
    assert_eq!(
        (&*events.holder_basis[0].value, &events.holder_basis[0].storage_contract),
        ("6000000", &m.comet)
    );
    assert_eq!(events.global_state.len(), 4);
    assert!(events.epochs.is_empty());
    // Without the guard in the reviewed set the same block would fail closed.
    let unguarded = mutate(&|v| v["markets"][0]["other_slot_names"] = serde_json::json!([])).unwrap();
    assert!(project(&b, &unguarded).unwrap_err().to_string().contains("unresolved"));
    // A system-call write to a Comet word carries the SYSTEM_CALL scope.
    let mut b = block(10);
    b.system_calls = vec![eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![write(m.indices_slot, w(1), w(2), 5)],
        ..Default::default()
    }];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.global_state[0].scope, pb::Scope::SystemCall as i32);
    assert!(events.global_state[0].transaction_hash.is_empty());
}

#[test]
fn activation_emits_the_binding_and_the_implementation_immutables() {
    let events = project(&block(1), &config()).unwrap();
    assert_eq!(events.epochs.len(), 1);
    let e = &events.epochs[0];
    assert_eq!(
        (e.family, e.basis_kind, e.basis_signed, e.basis_bit_width, e.basis_carryover, e.global_carryover),
        (
            pb::ModelFamily::CompoundV3Comet as i32,
            pb::BasisKind::SignedPrincipal as i32,
            true,
            104,
            true,
            false
        )
    );
    assert_eq!((&*e.basis_scale, e.balance_decimals), (BASE_INDEX_SCALE, 6));
    assert_eq!(events.dependencies.len(), 2);
    let constants: Vec<_> = events
        .global_state
        .iter()
        .filter(|g| g.observation == pb::Observation::QualifiedConstant as i32)
        .collect();
    assert_eq!(constants.len(), 11);
    let kink = constants.iter().find(|g| g.field == pb::StateField::CometSupplyKink as i32).unwrap();
    assert_eq!(
        (&*kink.value, &*kink.scale, kink.boundary),
        ("800000000000000000", FACTOR_SCALE, pb::Boundary::Declaration as i32)
    );
    let base_scale = constants.iter().find(|g| g.field == pb::StateField::CometBaseScale as i32).unwrap();
    assert_eq!(base_scale.value, "1000000");
    assert_eq!(events.clocks[0].global_state_count, 11);
    let events = project(&block(2), &config()).unwrap();
    assert!(events.epochs.is_empty() && events.global_state.is_empty());
    let events = project(&block(1001), &config()).unwrap();
    assert_eq!(events.epochs[0].kind, pb::EpochEventKind::Reaffirmed as i32);
}

#[test]
fn pointer_and_code_changes_invalidate_with_evidence_and_the_block_keeps_decoding() {
    let m = market();
    let cfg = config();
    let mut b = block(10);
    let mut call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    call.storage_changes
        .push(write(m.implementation_slot, word(&m.implementation).unwrap(), w(0xbeef), 11));
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
    assert_eq!(events.epochs.len(), 1);
    let e = &events.epochs[0];
    assert_eq!(
        (e.kind, e.reason, &e.evidence_slot, &e.evidence_word),
        (
            pb::EpochEventKind::Invalidated as i32,
            pb::InvalidationReason::ImplementationPointerWrite as i32,
            &m.implementation_slot.to_vec(),
            &w(0xbeef).to_vec()
        )
    );
    assert_eq!((e.ordinal, e.transaction_index, &e.evidence_contract), (11, 9, &m.comet));
    // The upgrade write that lands on the bound implementation is the binding, not an invalidation.
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![write(m.implementation_slot, w(0xdead), word(&m.implementation).unwrap(), 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap().epochs.is_empty());
    for address in [&m.comet, &m.implementation] {
        b.transaction_traces = vec![tx(eth::Call {
            code_changes: vec![eth::CodeChange {
                address: address.clone(),
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
            (1, pb::InvalidationReason::CodeChange as i32, &vec![2; 32])
        );
    }
}

#[test]
fn reviewed_mappings_are_ignored_and_unknown_writes_fail_closed() {
    let m = market();
    let cfg = config();
    let mut b = block(10);
    // isAllowed[owner][manager] under a reviewed mapping base (slot 3).
    let owner_key = mapping_key(&[4; 20], &m.other_mapping_slots[1]);
    let manager_key = mapping_key(&[5; 20], &owner_key);
    let mut p2 = vec![0u8; 64];
    p2[12..32].copy_from_slice(&[5; 20]);
    p2[32..].copy_from_slice(&owner_key);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        keccak_preimages: [preimage(&[4; 20], &m.other_mapping_slots[1]), (hex::encode(manager_key), hex::encode(p2))].into(),
        storage_changes: vec![write(manager_key, [0; 32], pack(&[(0, 8, 1)]), 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap().global_state.is_empty());
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![write([0x99; 32], [0; 32], pack(&[(0, 8, 1)]), 10)],
        ..Default::default()
    })];
    let err = project(&b, &cfg).unwrap_err().to_string();
    assert!(err.contains("unresolved") && err.contains(COMET) && err.contains(&"99".repeat(32)));
}

#[test]
fn reverted_frames_failed_transactions_and_ordering_faults_are_handled() {
    let m = market();
    let cfg = config();
    let mut b = block(10);
    let mut call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    call.state_reverted = true;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
        let mut failed = tx(user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10));
        failed.calls[0].storage_changes.push(write(m.totals_slot, w(1), w(2), 11));
        failed.status = status as i32;
        b.transaction_traces = vec![failed];
        let events = project(&b, &cfg).unwrap();
        assert!(events.holder_basis.is_empty() && events.global_state.is_empty());
        assert_eq!(events.clocks[0].holder_basis_count, 0);
    }
    let mut unknown = tx(eth::Call::default());
    unknown.status = 0;
    b.transaction_traces = vec![unknown];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("incomplete transaction"));
    let mut empty = tx(eth::Call::default());
    empty.calls.clear();
    b.transaction_traces = vec![empty];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("incomplete transaction"));
    // Discontinuity and tie name the contract, key and ordinals.
    let mut call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    let key: [u8; 32] = call.storage_changes[0].key.clone().try_into().unwrap();
    call.storage_changes.push(write(key, principal_word(3, 0), principal_word(4, 0), 11));
    b.transaction_traces = vec![tx(call.clone())];
    let err = project(&b, &cfg).unwrap_err().to_string();
    assert!(err.contains("discontinuous") && err.contains(COMET) && err.contains(&hex::encode(key)) && err.contains("ordinal 11"));
    call.storage_changes[1].old_value = principal_word(2, 0).to_vec();
    call.storage_changes[1].ordinal = 10;
    b.transaction_traces = vec![tx(call)];
    let err = project(&b, &cfg).unwrap_err().to_string();
    assert!(err.contains("ambiguous") && err.contains("ordinal 10 repeats after 10"));
}

#[test]
fn validate_block_refuses_every_malformed_identity() {
    let cfg = config();
    type Mutation = Box<dyn Fn(&mut eth::Block)>;
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "Extended blocks required",
            Box::new(|b| b.detail_level = eth::block::DetailLevel::DetaillevelBase as i32),
        ),
        ("producer version", Box::new(|b| b.ver = 3)),
        ("producer version", Box::new(|b| b.ver = 4)), // the fixture lists version 5 only
        ("missing header", Box::new(|b| b.header = None)),
        ("invalid block identity", Box::new(|b| b.hash = vec![1; 31])),
        ("invalid block identity", Box::new(|b| b.header.as_mut().unwrap().parent_hash = vec![])),
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
    let mut b = block(0);
    b.header.as_mut().unwrap().number = 0;
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("header number mismatch"));
    // Version 4 is accepted when listed.
    let both = mutate(&|v| v["producer_versions"] = serde_json::json!([4, 5])).unwrap();
    let mut b = block(10);
    b.ver = 4;
    assert_eq!(project(&b, &both).unwrap().clocks[0].producer_version, 4);
}

#[test]
fn blocks_before_activation_emit_only_the_clock_and_markets_are_attributed_by_address() {
    let cfg = mutate(&|v| v["markets"][0]["activation_block"] = 100.into()).unwrap();
    let m = cfg.markets[0].clone();
    let mut b = block(50);
    let mut call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    call.storage_changes.push(write([0x99; 32], w(0), w(1), 11));
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty() && events.epochs.is_empty());
    assert_eq!((events.clocks.len(), events.clocks[0].holder_basis_count), (1, 0));
    let mut b = block(100);
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10))];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((events.holder_basis.len(), events.epochs[0].kind), (1, pb::EpochEventKind::Bound as i32));
    // Two Comets share slot numbers; rows are attributed by storage address.
    let two = mutate(&|v| {
        let mut second = v["markets"][0].clone();
        second["comet"] = format!("0x{CWETH}").into();
        second["base_token"] = format!("0x{WETH}").into();
        second["base_decimals"] = 18.into();
        second["immutables"]["base_scale"] = "1000000000000000000".into();
        second["implementation"] = "0x0000000000000000000000000000000000000002".into();
        v["markets"].as_array_mut().unwrap().push(second);
    })
    .unwrap();
    let cweth = hex::decode(CWETH).unwrap();
    let mut b = block(10);
    let usdc_call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    let weth_call = eth::Call {
        index: 2,
        address: cweth.clone(),
        keccak_preimages: [preimage(&[9; 20], &m.user_basic_slot)].into(),
        storage_changes: vec![write_at(
            &cweth,
            mapping_key(&[9; 20], &m.user_basic_slot),
            principal_word(0, 0),
            principal_word(-7, 0),
            11,
        )],
        ..Default::default()
    };
    b.transaction_traces = vec![tx(usdc_call), tx(weth_call)];
    let events = project(&b, &two).unwrap();
    assert_eq!(events.holder_basis.len(), 2);
    let by_market: Vec<(&Vec<u8>, &str)> = events.holder_basis.iter().map(|h| (&h.market, h.value.as_str())).collect();
    assert_eq!(by_market, vec![(&cweth, "-7"), (&m.comet, "2")]);
    let bound = project(&block(1), &two).unwrap();
    let decimals: Vec<u32> = bound.epochs.iter().map(|e| e.balance_decimals).collect();
    assert_eq!(decimals, vec![18, 6]);
    assert!(mutate(&|v| {
        let dup = v["markets"][0].clone();
        v["markets"].as_array_mut().unwrap().push(dup);
    })
    .unwrap_err()
    .to_string()
    .contains("duplicate"));
}

#[test]
fn output_is_deterministic_under_input_permutation() {
    let m = market();
    let cfg = config();
    let mut b = block(10);
    let call_a = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    let mut call_b = user_basic_call(&[8; 20], principal_word(5, 0), principal_word(6, 0), 12);
    call_b.storage_changes.push(write(m.indices_slot, w(1), w(2), 13));
    call_b.storage_changes.push(write(m.totals_slot, w(3), w(4), 14));
    let mut tx_b = tx(call_b);
    tx_b.index = 10;
    tx_b.hash = vec![8; 32];
    b.transaction_traces = vec![tx(call_a), tx_b];
    let forward = project(&b, &cfg).unwrap();
    let mut reversed = b.clone();
    reversed.transaction_traces.reverse();
    for t in &mut reversed.transaction_traces {
        for c in &mut t.calls {
            c.storage_changes.reverse();
        }
    }
    let backward = project(&reversed, &cfg).unwrap();
    assert_eq!(forward.encode_to_vec(), backward.encode_to_vec());
    assert_eq!(forward.holder_basis.iter().map(|h| h.holder[0]).collect::<Vec<_>>(), vec![8, 9]);
}

#[test]
fn parameters_are_explicit_and_fail_closed() {
    assert!(parse(r#"{"chain_id":1,"producer_versions":[5],"markets":[]}"#).unwrap().markets.is_empty());
    assert!(parse(r#"{"chain_id":1,"producer_versions":[]}"#).is_err());
    assert!(parse(r#"{"chain_id":1,"producer_versions":[3]}"#).unwrap_err().to_string().contains("4 and 5"));
    assert!(parse(r#"{"chain_id":1,"producer_versions":[5,6]}"#).is_err());
    let err = |f: &dyn Fn(&mut serde_json::Value)| mutate(f).map(|_| ()).unwrap_err().to_string();
    assert!(err(&|v| v["markets"][0]["immutables"]["supply_kink"] = "0x1".into()).contains("decimal"));
    assert!(err(&|v| v["markets"][0]["immutables"]["supply_kink"] = "007".into()).contains("decimal"));
    assert!(err(&|v| v["markets"][0]["immutables"]["base_index_scale"] = "1000000000000000000".into()).contains("BASE_INDEX_SCALE"));
    assert!(err(&|v| v["markets"][0]["immutables"]["factor_scale"] = "1000000000000000".into()).contains("FACTOR_SCALE"));
    assert!(err(&|v| v["markets"][0]["immutables"]["base_scale"] = "1000000000".into()).contains("10^base_decimals"));
    assert!(err(&|v| v["markets"][0]["base_decimals"] = 19.into()).contains("MAX_BASE_DECIMALS"));
    assert!(err(&|v| v["markets"][0]["totals_slot"] = v["markets"][0]["indices_slot"].clone()).contains("overlap"));
    assert!(err(&|v| v["markets"][0]["other_slots"] = serde_json::json!([format!("0x{GUARD}")])).contains("overlap"));
    assert!(err(&|v| v["markets"][0]["other_slot_names"] = serde_json::json!([""])).contains("empty slot name"));
    assert!(err(&|v| v["markets"][0]["immutables"]["extra"] = "1".into()).contains("unknown field"));
    assert!(err(&|v| v["markets"][0]["source_pin"] = "".into()).contains("source_pin"));
}

#[test]
fn bit_helpers_decode_signed_and_unsigned_ranges() {
    let w = principal_word(-1, 3);
    assert_eq!(signed_bits(&w, 0, 104).to_string(), "-1");
    assert_eq!(bits(&w, 104, 64).to_string(), "3");
    let w = principal_word(42, 0);
    assert_eq!(signed_bits(&w, 0, 104).to_string(), "42");
    assert_eq!(bits(&pack(&[(248, 8, 0xab)]), 248, 8).to_string(), "171");
}
