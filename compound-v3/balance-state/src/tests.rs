use super::*;

const EPOCH: &str = include_str!("../tests/fixtures/mainnet-cusdcv3-epoch.json");
const COMET: &str = "c3d688b66703497daa19211eedff47f25384cdc3";

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
fn write(key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: hex::decode(COMET).unwrap(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
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
fn user_basic_call(holder: &[u8], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::Call {
    let m = market();
    let key = mapping_key(holder, &m.user_basic_slot);
    let mut preimage = vec![0u8; 64];
    preimage[12..32].copy_from_slice(holder);
    preimage[32..].copy_from_slice(&m.user_basic_slot);
    eth::Call {
        index: 1,
        address: m.comet.clone(),
        keccak_preimages: [(hex::encode(key), hex::encode(preimage))].into(),
        storage_changes: vec![write(key, old, new, ordinal)],
        ..Default::default()
    }
}
/// int104 two's complement inside the low 104 bits.
fn principal_word(principal: i128, tracking_index: u128) -> [u8; 32] {
    let unsigned = if principal < 0 { (1u128 << 104) as i128 + principal } else { principal } as u128;
    pack(&[(0, 104, unsigned), (104, 64, tracking_index)])
}

#[test]
fn signed_principal_keeps_its_sign_and_ignores_tracking_fields() {
    let mut b = block(10);
    let old = principal_word(5_000_000, 77);
    let new = principal_word(-2_500_000, 78);
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], old, new, 10))];
    let events = project(&b, &config()).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
    let h = &events.holder_basis[0];
    assert_eq!((&*h.previous_value, &*h.value, h.signed, h.bit_width), ("5000000", "-2500000", true, 104));
    assert_eq!(h.basis_kind, pb::BasisKind::SignedPrincipal as i32);
    // A write that only moves tracking fields is not a holder update.
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(5, 1), principal_word(5, 2), 10))];
    assert!(project(&b, &config()).unwrap().holder_basis.is_empty());
    // int104 extremes round-trip.
    let min = -(1i128 << 103);
    let max = (1i128 << 103) - 1;
    b.transaction_traces = vec![tx(user_basic_call(&[9; 20], principal_word(max, 0), principal_word(min, 0), 10))];
    let events = project(&b, &config()).unwrap();
    assert_eq!(
        (&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value),
        ("10141204801825835211973625643007", "-10141204801825835211973625643008")
    );
}

#[test]
fn market_words_split_into_indices_totals_clock_and_flags() {
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
    let fields: Vec<(i32, String, String)> = events
        .global_state
        .iter()
        .map(|g| (g.field, g.previous_value.clone(), g.value.clone()))
        .collect();
    assert_eq!(
        fields,
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
            (pb::StateField::CometTotalBorrowBase as i32, "987654321098765".into(), "990000000000000".into()),
            (pb::StateField::CometLastAccrualTime as i32, "1789689000".into(), "1789689600".into()),
        ]
    );
    // Unchanged total supply and pause flags are not carried; no holder rows.
    assert!(events.holder_basis.is_empty());
    assert_eq!(events.global_state[0].scale, BASE_INDEX_SCALE);
    assert_eq!(events.global_state[0].bit_width, 64);
    assert_eq!(events.global_state[3].bit_offset, 208);
    assert_eq!(events.clocks[0].global_state_count, 4);
}

#[test]
fn activation_emits_the_binding_and_the_implementation_immutables() {
    let events = project(&block(1), &config()).unwrap();
    assert_eq!(events.epochs.len(), 1);
    let e = &events.epochs[0];
    assert_eq!(
        (e.family, e.basis_kind, e.basis_signed, e.basis_bit_width),
        (pb::ModelFamily::CompoundV3Comet as i32, pb::BasisKind::SignedPrincipal as i32, true, 104)
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
    // Not on the heartbeat: nothing but the clock.
    let events = project(&block(2), &config()).unwrap();
    assert!(events.epochs.is_empty() && events.global_state.is_empty());
    let events = project(&block(1001), &config()).unwrap();
    assert_eq!(events.epochs[0].kind, pb::EpochEventKind::Reaffirmed as i32);
}

#[test]
fn reviewed_mappings_are_ignored_and_unknown_writes_pointers_and_code_fail_closed() {
    let m = market();
    let cfg = config();
    let mut b = block(10);
    // isAllowed[owner][manager] under a reviewed mapping base (slot 3).
    let owner_key = mapping_key(&[4; 20], &m.other_mapping_slots[1]);
    let manager_key = mapping_key(&[5; 20], &owner_key);
    let mut p1 = vec![0u8; 64];
    p1[12..32].copy_from_slice(&[4; 20]);
    p1[32..].copy_from_slice(&m.other_mapping_slots[1]);
    let mut p2 = vec![0u8; 64];
    p2[12..32].copy_from_slice(&[5; 20]);
    p2[32..].copy_from_slice(&owner_key);
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        keccak_preimages: [(hex::encode(owner_key), hex::encode(p1)), (hex::encode(manager_key), hex::encode(p2))].into(),
        storage_changes: vec![write(manager_key, [0; 32], pack(&[(0, 8, 1)]), 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap().global_state.is_empty());
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![write([0x99; 32], [0; 32], pack(&[(0, 8, 1)]), 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("unresolved"));
    b.transaction_traces = vec![tx(eth::Call {
        address: m.comet.clone(),
        storage_changes: vec![write(m.implementation_slot, [1; 32], [2; 32], 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("pointer"));
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
        assert!(project(&b, &cfg).unwrap_err().to_string().contains("code changed"));
    }
    // Reverted frames and failed transactions emit nothing.
    let mut call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    call.state_reverted = true;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    // Tie and discontinuity.
    let mut call = user_basic_call(&[9; 20], principal_word(1, 0), principal_word(2, 0), 10);
    let key: [u8; 32] = call.storage_changes[0].key.clone().try_into().unwrap();
    call.storage_changes.push(write(key, principal_word(3, 0), principal_word(4, 0), 11));
    b.transaction_traces = vec![tx(call.clone())];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("discontinuous"));
    call.storage_changes[1].old_value = principal_word(2, 0).to_vec();
    call.storage_changes[1].ordinal = 10;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("ambiguous"));
}

#[test]
fn parameters_are_explicit_and_fail_closed() {
    assert!(parse(r#"{"chain_id":1,"producer_versions":[5],"markets":[]}"#).unwrap().markets.is_empty());
    assert!(parse(r#"{"chain_id":1,"producer_versions":[]}"#).is_err());
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    v["markets"][0]["immutables"]["supply_kink"] = "0x1".into();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("decimal"));
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    v["markets"][0]["totals_slot"] = v["markets"][0]["indices_slot"].clone();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("overlap"));
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    v["markets"][0]["immutables"]["extra"] = "1".into();
    assert!(parse(&v.to_string()).is_err());
    let mut b = block(10);
    b.ver = 4;
    assert!(project(&b, &config()).unwrap_err().to_string().contains("producer version"));
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
