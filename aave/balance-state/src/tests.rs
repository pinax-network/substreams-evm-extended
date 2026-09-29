use super::*;
use prost::Message;

const EPOCHS: &str = include_str!("../tests/fixtures/bsc-aave-v3-epochs.json");
const WITHDRAW: &[u8] = include_bytes!("../tests/fixtures/122288220-tx71-ausdt-withdraw.pb");
const SUPPLY: &[u8] = include_bytes!("../tests/fixtures/122288734-tx81-ausdt-supply.pb");
const TWO_RESERVES: &[u8] = include_bytes!("../tests/fixtures/122288932-tx1-ausdt-ausdc-two-reserves.pb");
const AUSDT: &str = "a9251ca9de909cb71783723713b21e4233fbf1b1";
const AUSDC: &str = "00901a076785e0906d1028c7d6372d247bec7d61";
const USDT: &str = "55d398326f99059ff775485246999027b3197955";
const POOL: &str = "6807dc923806fe8fd134338eabca509979a7e0cb";

fn config() -> Config {
    parse(EPOCHS).unwrap()
}
fn decode(bytes: &[u8]) -> eth::Block {
    let block = eth::Block::decode(bytes).unwrap();
    assert_eq!((block.ver, block.transaction_traces.len()), (5, 1));
    block
}
fn global<'a>(events: &'a pb::Events, market: &str, field: pb::StateField) -> &'a pb::GlobalState {
    events
        .global_state
        .iter()
        .find(|g| hex::encode(&g.market) == market && g.field == field as i32)
        .unwrap()
}
fn synthetic_block(number: u64) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp { seconds: 1789600000, nanos: 0 }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn tx(call: eth::Call) -> eth::TransactionTrace {
    eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 3,
        calls: vec![call],
        ..Default::default()
    }
}
fn write(address: &str, key: [u8; 32], old: u8, new: u8, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: hex::decode(address).unwrap(),
        key: key.to_vec(),
        old_value: vec![old],
        new_value: vec![new],
        ordinal,
    }
}
fn user_state_call(holder: &[u8], old: u8, new: u8, ordinal: u64) -> eth::Call {
    let market = &config().markets[0];
    let key = mapping_key(holder, &market.user_state_slot);
    let mut preimage = vec![0u8; 64];
    preimage[12..32].copy_from_slice(holder);
    preimage[32..].copy_from_slice(&market.user_state_slot);
    eth::Call {
        index: 1,
        address: hex::decode(AUSDT).unwrap(),
        keccak_preimages: [(hex::encode(key), hex::encode(preimage))].into(),
        storage_changes: vec![write(AUSDT, key, old, new, ordinal)],
        ..Default::default()
    }
}

#[test]
fn captured_withdrawal_reduces_holder_and_reserve_writes_to_end_of_block_rows() {
    let block = decode(WITHDRAW);
    let events = project(&block, &config()).unwrap();
    assert_eq!(events.clocks.len(), 1);
    assert_eq!(
        (events.clocks[0].number, events.clocks[0].timestamp, events.clocks[0].producer_version),
        (122288220, 1789591698, 5)
    );
    assert_eq!(events.clocks[0].parameters_sha256, config().parameters_sha256);
    // The holder's word was written twice (index then balance); one row.
    assert_eq!(events.holder_basis.len(), 1);
    let h = &events.holder_basis[0];
    assert_eq!(hex::encode(&h.market), AUSDT);
    assert_eq!(hex::encode(&h.holder), "e88a4b3c49d4bf48d938cee1400e6806b2550386");
    assert_eq!((&*h.previous_value, &*h.value), ("4915017801535918613", "0"));
    assert_eq!(
        (h.first_ordinal, h.ordinal, h.change_count, h.transaction_index, h.call_index),
        (3344, 3346, 2, 71, 11)
    );
    assert_eq!(
        (h.bit_offset, h.bit_width, h.signed, h.basis_kind),
        (0, 120, false, pb::BasisKind::ScaledBalance as i32)
    );
    assert_eq!(h.raw_word.len(), 32);
    // A known zero is emitted as "0"; the raw word still carries the index.
    assert_ne!(h.raw_word, vec![0u8; 32]);
    // Reserve inputs from Pool storage: index and rate share one slot, the
    // clock lives in the third word; scaled total supply on the aToken.
    let index = global(&events, AUSDT, pb::StateField::AaveLiquidityIndex);
    assert_eq!(
        (&*index.previous_value, &*index.value),
        ("1124514677823054194978648271", "1124514855764725683677494165")
    );
    assert_eq!((index.bit_offset, index.bit_width, &*index.scale, index.change_count), (0, 128, RAY, 2));
    assert_eq!(hex::encode(&index.key), USDT);
    assert_eq!(hex::encode(&index.storage_contract), POOL);
    let rate = global(&events, AUSDT, pb::StateField::AaveCurrentLiquidityRate);
    assert_eq!((&*rate.value, rate.bit_offset, rate.bit_width), ("28034916092215127902773292", 128, 128));
    assert_eq!(rate.storage_slot, index.storage_slot);
    let clock = global(&events, AUSDT, pb::StateField::AaveLastUpdateTimestamp);
    assert_eq!(
        (&*clock.previous_value, &*clock.value, clock.bit_offset, clock.bit_width),
        ("1789591520", "1789591698", 128, 40)
    );
    assert_eq!(clock.value, events.clocks[0].timestamp.to_string());
    let supply = global(&events, AUSDT, pb::StateField::AaveScaledTotalSupply);
    assert_eq!(
        (&*supply.previous_value, &*supply.value),
        ("53319556175060462475822978", "53319551260042660939904365")
    );
    assert_eq!(hex::encode(&supply.storage_contract), AUSDT);
    assert_eq!((events.clocks[0].holder_basis_count, events.clocks[0].global_state_count), (1, 4));
    assert!(events.epochs.is_empty() && events.dependencies.is_empty());
}

#[test]
fn captured_supply_and_two_reserve_updates_keep_markets_separate() {
    let events = project(&decode(SUPPLY), &config()).unwrap();
    let h = &events.holder_basis[0];
    assert_eq!(hex::encode(&h.holder), "696b456c1c79416cce302d09e935b3cb80d0cdc5");
    assert_eq!((&*h.previous_value, &*h.value), ("11188775621800875963977379", "11188597767361778657139811"));
    assert_eq!(global(&events, AUSDT, pb::StateField::AaveLiquidityIndex).value, "1124515086691634348902975804");

    let events = project(&decode(TWO_RESERVES), &config()).unwrap();
    assert_eq!(events.holder_basis.len(), 2);
    let markets: Vec<String> = events.holder_basis.iter().map(|h| hex::encode(&h.market)).collect();
    assert_eq!(markets, vec![AUSDC, AUSDT]);
    assert!(events
        .holder_basis
        .iter()
        .all(|h| hex::encode(&h.holder) == "e5ec006540be4f7cbb2cbc7be79708a6d96f90dc"));
    assert_eq!(
        (&*events.holder_basis[0].previous_value, &*events.holder_basis[0].value),
        ("3288771638722128610456", "76617606431862")
    );
    assert_eq!(
        (&*events.holder_basis[1].previous_value, &*events.holder_basis[1].value),
        ("27228238249923106445252", "45954312872751152145397")
    );
    assert_eq!(global(&events, AUSDC, pb::StateField::AaveLiquidityIndex).value, "1124367101626237618641033796");
    assert_eq!(global(&events, AUSDC, pb::StateField::AaveLiquidityIndex).change_count, 3);
    assert_eq!(global(&events, AUSDC, pb::StateField::AaveLastUpdateTimestamp).previous_value, "1789591264");
    assert_eq!(
        global(&events, AUSDT, pb::StateField::AaveLiquidityIndex).previous_value,
        "1124515086691634348902975804"
    );
    assert_eq!(events.clocks[0].global_state_count, 8);
    // Rows are sorted by market, then field, then key.
    let order: Vec<(String, i32)> = events.global_state.iter().map(|g| (hex::encode(&g.market), g.field)).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
}

#[test]
fn activation_block_emits_bound_epoch_and_dependencies_and_heartbeat_reaffirms() {
    let mut block = synthetic_block(122288006);
    let events = project(&block, &config()).unwrap();
    assert_eq!(events.epochs.len(), 2);
    assert!(events.epochs.iter().all(|e| e.kind == pb::EpochEventKind::Bound as i32));
    let usdt = events.epochs.iter().find(|e| hex::encode(&e.market) == AUSDT).unwrap();
    assert_eq!(
        (&*usdt.model_id, &*usdt.basis_scale, usdt.balance_rounding, usdt.basis_bit_width),
        ("aave-v3/atoken/scaled-floor", RAY, pb::Rounding::Floor as i32, 120)
    );
    assert_eq!(hex::encode(&usdt.balance_asset), USDT);
    assert_eq!(events.dependencies.len(), 6);
    let pool = events.dependencies.iter().find(|d| d.role == pb::DependencyRole::Pool as i32).unwrap();
    assert_eq!(hex::encode(&pool.contract), POOL);
    assert_eq!(pool.binding, pb::BindingKind::StoragePointer as i32);
    assert_eq!(hex::encode(&pool.pointer_value[12..]), "5e2b0fcc5b9734c7ec0a03401ee9e6805f783b6d");
    assert_eq!((events.clocks[0].epoch_count, events.clocks[0].dependency_count), (2, 6));
    // Heartbeat: 1000 blocks later.
    block.number = 122289006;
    block.header.as_mut().unwrap().number = 122289006;
    let events = project(&block, &config()).unwrap();
    assert!(events.epochs.iter().all(|e| e.kind == pb::EpochEventKind::Reaffirmed as i32));
    block.number = 122289007;
    block.header.as_mut().unwrap().number = 122289007;
    assert!(project(&block, &config()).unwrap().epochs.is_empty());
    // Before activation the market is not bound: only the clock is emitted.
    block.number = 122288005;
    block.header.as_mut().unwrap().number = 122288005;
    block.transaction_traces = vec![tx(user_state_call(&[9; 20], 1, 2, 10))];
    let events = project(&block, &config()).unwrap();
    assert!(events.holder_basis.is_empty() && events.epochs.is_empty());
    assert_eq!(events.clocks.len(), 1);
}

#[test]
fn synthetic_holder_write_is_reduced_with_continuity_and_masked_to_the_basis_bits() {
    let mut block = synthetic_block(122288100);
    let mut call = user_state_call(&[9; 20], 1, 2, 10);
    // A second write in the same call continues from the first; the high
    // 128 bits (additionalData) are not part of the basis.
    let key = call.storage_changes[0].key.clone();
    let mut second = write(AUSDT, key.clone().try_into().unwrap(), 2, 5, 11);
    second.old_value = vec![2];
    let mut with_index = vec![0u8; 32];
    with_index[0] = 0xff; // additionalData bits
    with_index[31] = 5;
    second.new_value = with_index;
    call.storage_changes.push(second);
    block.transaction_traces = vec![tx(call)];
    let events = project(&block, &config()).unwrap();
    let h = &events.holder_basis[0];
    assert_eq!(
        (&*h.previous_value, &*h.value, h.first_ordinal, h.ordinal, h.change_count),
        ("1", "5", 10, 11, 2)
    );
    // Discontinuity or a tie fails the block.
    block.transaction_traces[0].calls[0].storage_changes[1].old_value = vec![3];
    assert!(project(&block, &config()).unwrap_err().to_string().contains("discontinuous"));
    block.transaction_traces[0].calls[0].storage_changes[1].old_value = vec![2];
    block.transaction_traces[0].calls[0].storage_changes[1].ordinal = 10;
    assert!(project(&block, &config()).unwrap_err().to_string().contains("ambiguous"));
}

#[test]
fn reverted_writes_and_failed_transactions_emit_nothing() {
    let mut block = synthetic_block(122288100);
    let mut call = user_state_call(&[9; 20], 1, 2, 10);
    call.state_reverted = true;
    block.transaction_traces = vec![tx(call)];
    assert!(project(&block, &config()).unwrap().holder_basis.is_empty());
    let mut failed = tx(user_state_call(&[9; 20], 1, 2, 10));
    failed.status = eth::TransactionTraceStatus::Reverted as i32;
    failed.calls[0].state_reverted = true;
    block.transaction_traces = vec![failed];
    assert!(project(&block, &config()).unwrap().holder_basis.is_empty());
}

#[test]
fn unresolved_atoken_writes_fail_closed_and_pointer_writes_and_code_changes_invalidate() {
    let cfg = config();
    let market = &cfg.markets[0];
    let mut block = synthetic_block(122288100);
    // Unknown aToken slot without a preimage.
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![write(AUSDT, [0x99; 32], 0, 1, 10)],
        ..Default::default()
    })];
    assert!(project(&block, &cfg).unwrap_err().to_string().contains("unresolved"));
    // Reviewed allowance mapping (slot 0x35) is ignored when its preimage chain
    // is complete.
    let owner_key = mapping_key(&[4; 20], &market.other_mapping_slots[0]);
    let spender_key = mapping_key(&[5; 20], &owner_key);
    let mut p1 = vec![0u8; 64];
    p1[12..32].copy_from_slice(&[4; 20]);
    p1[32..].copy_from_slice(&market.other_mapping_slots[0]);
    let mut p2 = vec![0u8; 64];
    p2[12..32].copy_from_slice(&[5; 20]);
    p2[32..].copy_from_slice(&owner_key);
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        keccak_preimages: [(hex::encode(owner_key), hex::encode(p1)), (hex::encode(spender_key), hex::encode(p2))].into(),
        storage_changes: vec![write(AUSDT, spender_key, 0, 1, 10)],
        ..Default::default()
    })];
    assert!(project(&block, &cfg).unwrap().holder_basis.is_empty());
    // Implementation pointer writes: another target invalidates the epoch with
    // evidence; a write onto the bound implementation is the binding itself.
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![write(AUSDT, market.implementation_slot, 1, 2, 10)],
        ..Default::default()
    })];
    let events = project(&block, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 1);
    let e = &events.epochs[0];
    assert_eq!(
        (e.kind, e.reason, &e.market, &e.evidence_slot, e.ordinal),
        (
            pb::EpochEventKind::Invalidated as i32,
            pb::InvalidationReason::ImplementationPointerWrite as i32,
            &market.atoken,
            &market.implementation_slot.to_vec(),
            10
        )
    );
    let mut onto_bound = eth::StorageChange {
        address: market.atoken.clone(),
        key: market.implementation_slot.to_vec(),
        old_value: vec![1],
        new_value: vec![],
        ordinal: 10,
    };
    onto_bound.new_value = {
        let mut w = vec![0u8; 12];
        w.extend_from_slice(&market.implementation);
        w
    };
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![onto_bound.clone()],
        ..Default::default()
    })];
    // STORAGE_POINTER contract: a write onto the bound implementation still
    // invalidates an active epoch; installing the epoch's own implementation
    // is expressed with `activation_ordinal` instead.
    assert_eq!(
        project(&block, &cfg).unwrap().epochs[0].reason,
        pb::InvalidationReason::ImplementationPointerWrite as i32
    );
    // An aToken excursion X -> Z -> X is evidenced write by write.
    let bound_word = onto_bound.new_value.clone();
    let mut rogue = vec![0u8; 32];
    rogue[31] = 0xbd;
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![
            eth::StorageChange {
                address: market.atoken.clone(),
                key: market.implementation_slot.to_vec(),
                old_value: bound_word.clone(),
                new_value: rogue.clone(),
                ordinal: 10,
            },
            eth::StorageChange {
                address: market.atoken.clone(),
                key: market.implementation_slot.to_vec(),
                old_value: rogue.clone(),
                new_value: bound_word.clone(),
                ordinal: 11,
            },
        ],
        ..Default::default()
    })];
    let excursion: Vec<(u64, Vec<u8>)> = project(&block, &cfg)
        .unwrap()
        .epochs
        .iter()
        .filter(|e| e.market == market.atoken)
        .map(|e| (e.ordinal, e.evidence_word.clone()))
        .collect();
    assert_eq!(excursion, vec![(10, rogue.clone()), (11, bound_word.clone())]);
    // An equal-value write is dropped as a balance effect but still invalidates.
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![eth::StorageChange {
            address: market.atoken.clone(),
            key: market.implementation_slot.to_vec(),
            old_value: bound_word.clone(),
            new_value: bound_word.clone(),
            ordinal: 10,
        }],
        ..Default::default()
    })];
    assert_eq!(project(&block, &cfg).unwrap().epochs.len(), 1);
    let pool = cfg.pool.as_ref().unwrap();
    block.transaction_traces = vec![tx(eth::Call {
        address: pool.address.clone(),
        storage_changes: vec![write(POOL, pool.implementation_slot, 1, 2, 10)],
        ..Default::default()
    })];
    let events = project(&block, &cfg).unwrap();
    assert_eq!(events.epochs.len(), cfg.markets.len());
    assert!(events
        .epochs
        .iter()
        .all(|e| e.reason == pb::InvalidationReason::DependencyPointerWrite as i32 && e.evidence_contract == pool.address));
    // Unrelated Pool writes are outside the bound reserve structs.
    block.transaction_traces = vec![tx(eth::Call {
        address: pool.address.clone(),
        storage_changes: vec![write(POOL, [0x77; 32], 1, 2, 10)],
        ..Default::default()
    })];
    assert!(project(&block, &cfg).unwrap().global_state.is_empty());
    // Code changes on any bound contract invalidate with the code hash as
    // evidence: the aToken and its implementation affect one market, the Pool
    // and its implementation every active market.
    for (address, reason, count) in [
        (&pool.address, pb::InvalidationReason::DependencyCodeChange, cfg.markets.len()),
        (&pool.implementation, pb::InvalidationReason::DependencyCodeChange, cfg.markets.len()),
        (&market.atoken, pb::InvalidationReason::CodeChange, 1),
        // aBnbUSDT and aBnbUSDC share one aToken implementation.
        (
            &market.implementation,
            pb::InvalidationReason::CodeChange,
            cfg.markets.iter().filter(|m| m.implementation == market.implementation).count(),
        ),
    ] {
        block.transaction_traces = vec![tx(eth::Call {
            code_changes: vec![eth::CodeChange {
                address: address.clone(),
                old_hash: vec![1; 32],
                new_hash: vec![2; 32],
                ordinal: 5,
                ..Default::default()
            }],
            ..Default::default()
        })];
        let events = project(&block, &cfg).unwrap();
        assert_eq!(events.epochs.len(), count);
        assert!(events
            .epochs
            .iter()
            .all(|e| e.reason == reason as i32 && e.evidence_code_hash == vec![2; 32] && e.kind == pb::EpochEventKind::Invalidated as i32));
    }
}

#[test]
fn reserve_struct_words_outside_the_model_are_recognized_and_ignored() {
    let cfg = config();
    let market = &cfg.markets[0];
    let pool = cfg.pool.as_ref().unwrap();
    let mut block = synthetic_block(122288100);
    let mut changes = vec![];
    for offset in [0u8, 2, 4, 8, 9] {
        changes.push(write(POOL, add_offset(&market.reserve_base, offset), 1, 2, 10 + offset as u64));
    }
    block.transaction_traces = vec![tx(eth::Call {
        address: pool.address.clone(),
        storage_changes: changes,
        ..Default::default()
    })];
    assert!(project(&block, &cfg).unwrap().global_state.is_empty());
    // Offset 1 and 3 produce the three bound fields.
    block.transaction_traces[0].calls[0].storage_changes = vec![
        write(POOL, add_offset(&market.reserve_base, 1), 1, 2, 10),
        write(POOL, add_offset(&market.reserve_base, 3), 1, 2, 11),
    ];
    let events = project(&block, &cfg).unwrap();
    let fields: Vec<i32> = events.global_state.iter().map(|g| g.field).collect();
    assert_eq!(
        fields,
        vec![
            pb::StateField::AaveLiquidityIndex as i32,
            pb::StateField::AaveCurrentLiquidityRate as i32,
            pb::StateField::AaveLastUpdateTimestamp as i32
        ]
    );
    assert_eq!(events.global_state[1].value, "0"); // rate lives in the high bits
    assert_eq!(events.global_state[2].value, "0"); // timestamp starts at bit 128
}

#[test]
fn bit_extraction_and_slot_arithmetic_are_exact() {
    let mut w = [0u8; 32];
    w[31] = 0b1011_0101;
    w[30] = 0x01;
    assert_eq!(bits(&w, 0, 8).to_string(), "181");
    assert_eq!(bits(&w, 8, 8).to_string(), "1");
    assert_eq!(bits(&w, 0, 120).to_string(), "437");
    let mut high = [0u8; 32];
    high[0] = 0x80;
    assert_eq!(bits(&high, 255, 1).to_string(), "1");
    assert_eq!(bits(&high, 0, 255).to_string(), "0");
    let base = [0xff; 32];
    assert_eq!(add_offset(&base, 1), [0u8; 32]); // wraps modulo 2^256
    let mut near = [0u8; 32];
    near[31] = 0xfe;
    assert_eq!(add_offset(&near, 3)[30..], [0x01, 0x01]);
    assert_eq!(struct_offset(&add_offset(&near, 9), &near, 10), Some(9));
    assert_eq!(struct_offset(&add_offset(&near, 10), &near, 10), None);
    let usdt = hex::decode(USDT).unwrap();
    assert_eq!(
        hex::encode(mapping_key(&usdt, &config().pool.as_ref().unwrap().reserves_slot)),
        "01e77548dd65884cfab89fc0b4723cfde5b99f1ffaee11e7a06bfd4142593ada"
    );
}

#[test]
fn parameters_are_explicit_and_fail_closed() {
    assert!(parse(r#"{"chain_id":56,"producer_versions":[5],"markets":[]}"#).unwrap().markets.is_empty());
    assert!(parse(r#"{"chain_id":56,"producer_versions":[]}"#).is_err());
    assert!(parse(r#"{"chain_id":0,"producer_versions":[5]}"#).is_err());
    assert!(parse(r#"{"chain_id":56,"producer_versions":[5],"unknown":1}"#).is_err());
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["pool"] = serde_json::Value::Null;
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("require a pool"));
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["rounding"] = "bankers".into();
    assert!(parse(&v.to_string()).is_err());
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["basis_bits"] = 96.into();
    assert!(parse(&v.to_string()).is_err());
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][1]["atoken"] = v["markets"][0]["atoken"].clone();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("duplicate"));
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["markets"][0]["total_supply_slot"] = v["markets"][0]["user_state_slot"].clone();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("overlap"));
    // Unqualified producer versions and non-Extended blocks are refused.
    let cfg = config();
    let mut block = synthetic_block(122288100);
    block.ver = 4;
    assert!(project(&block, &cfg).unwrap_err().to_string().contains("producer version"));
    block.ver = 5;
    block.detail_level = eth::block::DetailLevel::DetaillevelBase as i32;
    assert!(project(&block, &cfg).is_err());
}

#[test]
fn different_atokens_cannot_bind_the_same_pool_reserve() {
    let valid = config();
    assert_ne!(valid.markets[0].underlying, valid.markets[1].underlying);
    assert_ne!(valid.markets[0].reserve_base, valid.markets[1].reserve_base);
    for later_activation in [false, true] {
        for reversed in [false, true] {
            let mut raw: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
            // Compare decoded identities, not spelling or activation order.
            raw["markets"][1]["underlying"] = format!("0x{}", USDT.to_uppercase()).into();
            if later_activation {
                raw["markets"][1]["activation_block"] = 123_000_000.into();
                raw["markets"][1]["activation_ordinal"] = 100.into();
            }
            if reversed {
                raw["markets"].as_array_mut().unwrap().reverse();
            }
            let error = parse(&raw.to_string()).unwrap_err().to_string();
            assert!(error.contains("duplicate underlying/reserve binding within pool"), "{error}");
        }
    }
}

#[test]
fn every_fixture_carries_its_recorded_identity() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/cases.json")).unwrap();
    let listed = cases["cases"].as_array().unwrap();
    assert_eq!(listed.len(), 3);
    for (file, bytes) in [
        ("122288220-tx71-ausdt-withdraw.pb", WITHDRAW),
        ("122288734-tx81-ausdt-supply.pb", SUPPLY),
        ("122288932-tx1-ausdt-ausdc-two-reserves.pb", TWO_RESERVES),
    ] {
        let case = listed.iter().find(|c| c["file"] == file).unwrap();
        let block = decode(bytes);
        assert_eq!(case["block"], block.number);
        assert_eq!(case["block_hash"], format!("0x{}", hex::encode(&block.hash)));
        assert_eq!(case["transaction"], format!("0x{}", hex::encode(&block.transaction_traces[0].hash)));
        assert_eq!(case["fixture_sha256"], hex::encode(sha2::Sha256::digest(bytes)));
    }
}

#[test]
fn producer_versions_are_restricted_to_the_qualified_extended_versions() {
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["producer_versions"] = serde_json::json!([3]);
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("4 and 5"));
    v["producer_versions"] = serde_json::json!([4, 5]);
    assert!(parse(&v.to_string()).is_ok());
    v["producer_versions"] = serde_json::json!([]);
    assert!(parse(&v.to_string()).is_err());
}

#[test]
fn an_epoch_bound_mid_block_owns_only_effects_from_its_activation_ordinal() {
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    let activation = v["markets"][0]["activation_block"].as_u64().unwrap();
    v["markets"][0]["activation_ordinal"] = 100.into();
    let cfg = parse(&v.to_string()).unwrap();
    let market = cfg.markets[0].clone();
    let pool = cfg.pool.clone().unwrap();
    let mut block = synthetic_block(activation);
    // A Pool implementation write before this market's activation ordinal
    // (50) does not invalidate it, while the other market, active from
    // ordinal 0, is invalidated by the same write.
    block.transaction_traces = vec![tx(eth::Call {
        address: pool.address.clone(),
        storage_changes: vec![write(POOL, pool.implementation_slot, 1, 2, 50)],
        ..Default::default()
    })];
    let events = project(&block, &cfg).unwrap();
    let invalidated: Vec<&Vec<u8>> = events
        .epochs
        .iter()
        .filter(|e| e.kind == pb::EpochEventKind::Invalidated as i32)
        .map(|e| &e.market)
        .collect();
    assert!(!invalidated.contains(&&market.atoken));
    assert!(cfg.markets.iter().skip(1).all(|m| invalidated.contains(&&m.atoken)));
    let bound_row = events
        .epochs
        .iter()
        .find(|e| e.market == market.atoken && e.kind == pb::EpochEventKind::Bound as i32)
        .unwrap();
    assert_eq!((bound_row.activation_ordinal, bound_row.ordinal), (100, 100));
    // The same write after the activation ordinal invalidates it too.
    block.transaction_traces = vec![tx(eth::Call {
        address: pool.address.clone(),
        storage_changes: vec![write(POOL, pool.implementation_slot, 1, 2, 150)],
        ..Default::default()
    })];
    let events = project(&block, &cfg).unwrap();
    assert!(events
        .epochs
        .iter()
        .any(|e| e.market == market.atoken && e.kind == pb::EpochEventKind::Invalidated as i32));
    // An unresolved aToken write before activation belongs to the previous
    // epoch; after it, the block is refused.
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![write(AUSDT, [0x99; 32], 0, 1, 60)],
        ..Default::default()
    })];
    assert!(project(&block, &cfg).is_ok());
    block.transaction_traces = vec![tx(eth::Call {
        address: market.atoken.clone(),
        storage_changes: vec![write(AUSDT, [0x99; 32], 0, 1, 160)],
        ..Default::default()
    })];
    assert!(project(&block, &cfg).unwrap_err().to_string().contains("unresolved"));
}

#[test]
fn validate_block_refuses_every_malformed_identity_and_incomplete_transaction() {
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
        ("invalid block identity", Box::new(|b| b.header.as_mut().unwrap().state_root = vec![3; 33])),
        ("header number mismatch", Box::new(|b| b.header.as_mut().unwrap().number += 1)),
        ("missing timestamp", Box::new(|b| b.header.as_mut().unwrap().timestamp = None)),
        (
            "negative timestamp",
            Box::new(|b| b.header.as_mut().unwrap().timestamp.as_mut().unwrap().seconds = -1),
        ),
        (
            "incomplete transaction persistence data",
            Box::new(|b| b.transaction_traces[0].status = eth::TransactionTraceStatus::Unknown as i32),
        ),
        ("incomplete transaction persistence data", Box::new(|b| b.transaction_traces[0].calls.clear())),
    ];
    for (message, apply) in cases {
        let mut b = synthetic_block(122288100);
        b.transaction_traces = vec![tx(user_state_call(&[9; 20], 1, 2, 10))];
        apply(&mut b);
        let err = project(&b, &cfg).unwrap_err().to_string();
        assert!(err.contains(message), "expected `{message}`, got `{err}`");
    }
    let mut b = synthetic_block(0);
    b.header.as_mut().unwrap().number = 0;
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("header number mismatch"));
    // Version 4 is accepted when listed.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    v["producer_versions"] = serde_json::json!([4, 5]);
    let both = parse(&v.to_string()).unwrap();
    let mut b = synthetic_block(122288100);
    b.ver = 4;
    assert_eq!(project(&b, &both).unwrap().clocks[0].producer_version, 4);
}

#[test]
fn output_is_deterministic_under_input_permutation() {
    let cfg = config();
    let mut b = synthetic_block(122288100);
    let mut call_a = user_state_call(&[9; 20], 1, 2, 10);
    let second_holder = user_state_call(&[8; 20], 5, 6, 12);
    call_a.keccak_preimages.extend(second_holder.keccak_preimages.clone());
    call_a.storage_changes.extend(second_holder.storage_changes.clone());
    let mut tx_b = tx(user_state_call(&[7; 20], 3, 4, 20));
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
    assert_eq!(forward.holder_basis.iter().map(|h| h.holder[0]).collect::<Vec<_>>(), vec![7, 8, 9]);
}

fn word_of(slot: u8) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = slot;
    w
}

#[test]
fn a_permit_nonce_write_is_reviewed_storage_not_a_halted_block() {
    // Live BSC block 123068971: a router `permit` wrote aBnbUSDT
    // `_nonces[0xfb1f…943a]` (slot 58) and the package refused the block.
    let owner = hex::decode("fb1fcb75d8c1209fb4210b119dcb304dfea0943a").unwrap();
    let nonces = word_of(58);
    let key = mapping_key(&owner, &nonces);
    assert_eq!(hex::encode(key), "9803bfc9304d070a6ff21adc6321ecb448ae31ed5b840dd0c8ffb36d45e2a664");
    let mut preimage = vec![0u8; 64];
    preimage[12..32].copy_from_slice(&owner);
    preimage[32..].copy_from_slice(&nonces);
    let mut block = synthetic_block(122288100);
    block.transaction_traces = vec![tx(eth::Call {
        index: 1,
        address: hex::decode(AUSDT).unwrap(),
        keccak_preimages: [(hex::encode(key), hex::encode(preimage))].into(),
        storage_changes: vec![write(AUSDT, key, 3, 4, 10)],
        ..Default::default()
    })];
    let events = project(&block, &config()).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty() && events.epochs.is_empty());
    // Without the reviewed nonces mapping the same block fails closed, as it did live.
    let mut v: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    for m in v["markets"].as_array_mut().unwrap() {
        m["other_mapping_slots"] = serde_json::json!(["0x0000000000000000000000000000000000000000000000000000000000000035"]);
    }
    let error = project(&block, &parse(&v.to_string()).unwrap()).unwrap_err().to_string();
    assert!(error.contains("unresolved storage") && error.contains("9803bfc9"), "{error}");
}

#[test]
fn an_upgrade_ends_the_epoch_at_its_pointer_write_for_the_rest_of_the_block() {
    let cfg = config();
    let market = &cfg.markets[0];
    let pool = cfg.pool.clone().unwrap();
    // aToken upgrade: the pointer write (10) is evidence; `initialize` then
    // writes `lastInitializedRevision` (slot 0), which the epoch never
    // described, and a holder word, neither decoded under the ended epoch.
    let mut upgrade = user_state_call(&[9; 20], 1, 2, 12);
    upgrade.storage_changes.insert(0, write(AUSDT, market.implementation_slot, 1, 2, 10));
    upgrade.storage_changes.insert(1, write(AUSDT, word_of(0), 4, 5, 11));
    let mut block = synthetic_block(122288100);
    block.transaction_traces = vec![tx(upgrade.clone())];
    let events = project(&block, &cfg).unwrap();
    assert_eq!(
        events.epochs.iter().map(|e| (e.reason, e.ordinal)).collect::<Vec<_>>(),
        vec![(pb::InvalidationReason::ImplementationPointerWrite as i32, 10)]
    );
    assert!(events.holder_basis.is_empty());
    // The same unknown write before the pointer write still fails closed.
    block.transaction_traces[0].calls[0].storage_changes[1].ordinal = 9;
    assert!(project(&block, &cfg).unwrap_err().to_string().contains("unresolved storage"));
    // A Pool upgrade ends every market's epoch; later reserve words of the
    // block are not decoded under them.
    let reserve_word = add_offset(&market.reserve_base, 1);
    let mut block = synthetic_block(122288100);
    block.transaction_traces = vec![tx(eth::Call {
        index: 1,
        address: pool.address.clone(),
        storage_changes: vec![
            write(POOL, reserve_word, 1, 2, 9),
            write(POOL, pool.implementation_slot, 1, 2, 10),
            write(POOL, reserve_word, 2, 3, 11),
        ],
        ..Default::default()
    })];
    let events = project(&block, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 2);
    // The pre-upgrade index is not the end-of-block value either.
    assert!(events.global_state.is_empty());
    assert_eq!((events.clocks[0].global_state_count, events.clocks[0].epoch_count), (0, 2));
}

#[test]
fn invalidation_suppresses_partial_holder_state_but_preserves_other_markets() {
    let cfg = config();
    let market = &cfg.markets[0];
    let other = &cfg.markets[1];
    let mut before = user_state_call(&[9; 20], 1, 2, 9);
    before.storage_changes.extend([
        write(AUSDT, market.total_supply_slot, 5, 6, 8),
        // An equal-value pointer write still ends the epoch.
        write(AUSDT, market.implementation_slot, 1, 1, 10),
        // Unknown new-model storage at the exact cutoff is quarantined.
        write(AUSDT, word_of(0), 0, 1, 10),
    ]);
    let mut unaffected = user_state_call(&[8; 20], 3, 4, 11);
    unaffected.address = other.atoken.clone();
    unaffected.storage_changes[0].address = other.atoken.clone();
    unaffected
        .storage_changes
        .push(write(POOL, add_offset(&other.reserve_base, RESERVE_INDEX_RATE_OFFSET), 1, 2, 12));
    assert_eq!(market.user_state_slot, other.user_state_slot);
    let mut block = synthetic_block(122288100);
    block.transaction_traces = vec![tx(before), tx(unaffected)];
    let events = project(&block, &cfg).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
    assert_eq!(events.holder_basis[0].market, other.atoken);
    assert_eq!(events.holder_basis[0].value, "4");
    assert_eq!(events.global_state.len(), 2);
    assert!(events.global_state.iter().all(|g| g.market == other.atoken && g.key == other.underlying));
    let index = global(&events, AUSDC, pb::StateField::AaveLiquidityIndex);
    assert_eq!((&*index.previous_value, &*index.value, index.ordinal), ("1", "2", 12));
    assert_eq!(events.epochs.len(), 1);
    assert_eq!(events.clocks[0].holder_basis_count, 1);
    // Suppression must not conceal unknown writes owned by the old epoch.
    block.transaction_traces[0].calls[0].storage_changes[3].ordinal = 7;
    assert!(project(&block, &cfg).unwrap_err().to_string().contains("unresolved"));
}

#[test]
fn code_invalidation_omits_the_old_prefix_and_reverted_upgrades_do_not() {
    let cfg = config();
    let market = &cfg.markets[0];
    let mut block = synthetic_block(122288100);
    block.transaction_traces = vec![tx(user_state_call(&[9; 20], 1, 2, 9))];
    block.transaction_traces[0].calls.push(eth::Call {
        index: 2,
        address: market.atoken.clone(),
        code_changes: vec![eth::CodeChange {
            address: market.implementation.clone(),
            old_hash: vec![1; 32],
            new_hash: vec![2; 32],
            ordinal: 10,
            ..Default::default()
        }],
        storage_changes: vec![write(AUSDT, word_of(0), 0, 1, 10)],
        ..Default::default()
    });
    let events = project(&block, &cfg).unwrap();
    assert!(events.holder_basis.is_empty());
    assert_eq!(events.epochs[0].ordinal, 10);
    block.transaction_traces[0].calls[1].state_reverted = true;
    let events = project(&block, &cfg).unwrap();
    assert!(events.epochs.is_empty());
    assert_eq!(events.holder_basis[0].value, "2");
}

fn successor_params() -> serde_json::Value {
    let mut raw: serde_json::Value = serde_json::from_str(EPOCHS).unwrap();
    raw["heartbeat_blocks"] = 1.into();
    let mut next = raw["markets"][0].clone();
    next["epoch"] = 3.into();
    next["activation_block"] = (next["activation_block"].as_u64().unwrap() + 1).into();
    next["activation_ordinal"] = 30.into();
    raw["markets"].as_array_mut().unwrap().push(next);
    raw
}

#[test]
fn successor_schedule_splits_physical_effects_and_retires_prefix_rows() {
    let config = parse(&successor_params().to_string()).unwrap();
    let first = &config.markets[0];
    let number = first.activation_block + 1;
    let mut block = synthetic_block(number);
    let mut call = user_state_call(&[9; 20], 7, 9, 20);
    let key = call.storage_changes[0].key.clone();
    let mut suffix = call.storage_changes[0].clone();
    suffix.old_value = vec![9];
    suffix.new_value = vec![0];
    suffix.ordinal = 30;
    call.storage_changes.push(suffix);
    let index = add_offset(&first.reserve_base, RESERVE_INDEX_RATE_OFFSET);
    call.storage_changes.extend([write(POOL, index, 1, 2, 20), write(POOL, index, 2, 3, 30)]);
    call.storage_changes.push(write(AUSDC, config.markets[1].total_supply_slot, 3, 4, 40));
    block.transaction_traces.push(tx(call));
    let output = project(&block, &config).unwrap();
    let holder = &output.holder_basis[0];
    assert_eq!((holder.epoch, holder.first_ordinal, holder.ordinal, holder.change_count), (3, 30, 30, 1));
    assert_eq!((&*holder.previous_value, &*holder.value), ("9", "0"));
    assert_eq!(holder.storage_slot, key);
    assert!(output.global_state.iter().filter(|row| row.market == first.atoken).all(|row| row.epoch == 3));
    assert_eq!(global(&output, AUSDT, pb::StateField::AaveLiquidityIndex).previous_value, "2");
    assert_eq!(global(&output, AUSDC, pb::StateField::AaveScaledTotalSupply).value, "4");
    let bound = output.epochs.iter().find(|row| row.market == first.atoken && row.epoch == 3).unwrap();
    assert_eq!(
        (bound.kind, bound.ordinal, bound.basis_carryover, bound.global_carryover),
        (pb::EpochEventKind::Bound as i32, 30, false, false)
    );
    let quiet = project(&synthetic_block(number + 1), &config).unwrap();
    assert!(quiet
        .epochs
        .iter()
        .filter(|row| row.market == first.atoken)
        .all(|row| row.epoch == 3 && !row.basis_carryover && !row.global_carryover));
    assert!(quiet.dependencies.iter().filter(|row| row.market == first.atoken).all(|row| row.epoch == 3));
    assert!(quiet.holder_basis.is_empty() && quiet.global_state.is_empty());
}

#[test]
fn successor_schedule_never_hides_cross_boundary_discontinuity_or_unknown_prefixes() {
    let config = parse(&successor_params().to_string()).unwrap();
    let mut block = synthetic_block(config.markets[0].activation_block + 1);
    let mut call = user_state_call(&[9; 20], 7, 9, 20);
    let mut suffix = call.storage_changes[0].clone();
    suffix.old_value = vec![8];
    suffix.new_value = vec![10];
    suffix.ordinal = 30;
    call.storage_changes.push(suffix);
    block.transaction_traces.push(tx(call));
    assert!(project(&block, &config).unwrap_err().to_string().contains("discontinuous"));
    block.transaction_traces[0].calls[0].storage_changes = vec![write(AUSDT, [99; 32], 0, 1, 20)];
    assert!(project(&block, &config).unwrap_err().to_string().contains("unresolved"));
}

#[test]
fn successor_schedule_pointer_cutoffs_are_owned_by_exact_epoch() {
    let config = parse(&successor_params().to_string()).unwrap();
    let first = &config.markets[0];
    let mut block = synthetic_block(first.activation_block + 1);
    let mut call = user_state_call(&[9; 20], 7, 9, 30);
    call.storage_changes.push(write(AUSDT, first.implementation_slot, 1, 1, 20));
    block.transaction_traces.push(tx(call));
    let output = project(&block, &config).unwrap();
    assert_eq!(output.holder_basis[0].epoch, 3);
    assert!(output
        .epochs
        .iter()
        .any(|row| row.epoch == 1 && row.kind == pb::EpochEventKind::Invalidated as i32 && row.ordinal == 20));
    block.transaction_traces[0].calls[0].storage_changes[1].ordinal = 30;
    let output = project(&block, &config).unwrap();
    assert!(output.holder_basis.is_empty());
    assert!(output
        .epochs
        .iter()
        .any(|row| row.epoch == 3 && row.kind == pb::EpochEventKind::Invalidated as i32 && row.ordinal == 30));
}

#[test]
fn successor_schedule_accepts_unsorted_nonconsecutive_ids_and_rejects_ambiguity() {
    let raw = successor_params();
    let expected = parse(&raw.to_string()).unwrap();
    let mut reversed = raw.clone();
    reversed["markets"].as_array_mut().unwrap().reverse();
    let actual = parse(&reversed.to_string()).unwrap();
    let block = synthetic_block(expected.markets[0].activation_block + 1);
    let mut a = project(&block, &expected).unwrap();
    let mut b = project(&block, &actual).unwrap();
    a.clocks.clear();
    b.clocks.clear();
    assert_eq!(a.encode_to_vec(), b.encode_to_vec());
    for field in ["epoch", "activation_block"] {
        let mut bad = raw.clone();
        bad["markets"][2][field] = bad["markets"][0][field].clone();
        if field == "activation_block" {
            bad["markets"][2]["activation_ordinal"] = 0.into();
        }
        assert!(parse(&bad.to_string()).is_err());
    }
}

#[test]
fn successor_schedule_declares_every_in_block_epoch_without_partial_snapshots() {
    let mut raw = successor_params();
    let mut third = raw["markets"][2].clone();
    third["epoch"] = 8.into();
    third["activation_ordinal"] = 50.into();
    raw["markets"].as_array_mut().unwrap().push(third);
    let config = parse(&raw.to_string()).unwrap();
    let mut block = synthetic_block(config.markets[0].activation_block + 1);
    let mut call = user_state_call(&[9; 20], 1, 2, 20);
    for (old, new, ordinal) in [(2, 3, 30), (3, 0, 50)] {
        let mut w = call.storage_changes[0].clone();
        w.old_value = vec![old];
        w.new_value = vec![new];
        w.ordinal = ordinal;
        call.storage_changes.push(w);
    }
    block.transaction_traces.push(tx(call));
    let output = project(&block, &config).unwrap();
    assert_eq!(
        (output.holder_basis.len(), output.holder_basis[0].epoch, output.holder_basis[0].first_ordinal),
        (1, 8, 50)
    );
    assert_eq!(output.holder_basis[0].value, "0");
    for epoch in [3, 8] {
        assert!(output
            .epochs
            .iter()
            .any(|row| row.epoch == epoch && row.kind == pb::EpochEventKind::Bound as i32 && !row.basis_carryover && !row.global_carryover));
        assert!(output.dependencies.iter().any(|row| row.epoch == epoch));
    }
    // A zero-ordinal next-block start leaves no interval in that block for its predecessor.
    raw["markets"][2]["activation_ordinal"] = 0.into();
    let config = parse(&raw.to_string()).unwrap();
    let output = project(&synthetic_block(block.number), &config).unwrap();
    assert!(!output.epochs.iter().any(|row| row.market == config.markets[0].atoken && row.epoch == 1));
}

#[test]
fn successor_schedule_keeps_code_dependencies_and_post_cutoff_shape_checks_scoped() {
    let mut raw = successor_params();
    raw["markets"][2]["implementation"] = format!("0x{}", hex::encode([0xab; 20])).into();
    let config = parse(&raw.to_string()).unwrap();
    let first = &config.markets[0];
    for (ordinal, expected_epoch) in [(20, 1), (30, 3), (40, 3)] {
        let mut block = synthetic_block(first.activation_block + 1);
        let mut call = user_state_call(&[9; 20], 1, 2, 35);
        call.code_changes.push(eth::CodeChange {
            address: config.pool.as_ref().unwrap().address.clone(),
            ordinal,
            new_hash: vec![3; 32],
            ..Default::default()
        });
        block.transaction_traces.push(tx(call));
        let output = project(&block, &config).unwrap();
        assert!(output
            .epochs
            .iter()
            .any(|row| row.market == first.atoken && row.kind == pb::EpochEventKind::Invalidated as i32 && row.epoch == expected_epoch));
        assert_eq!(output.holder_basis.len(), usize::from(ordinal < 30));
    }
    let mut block = synthetic_block(first.activation_block + 1);
    let mut call = user_state_call(&[9; 20], 1, 2, 30);
    call.code_changes.push(eth::CodeChange {
        address: first.implementation.clone(),
        ordinal: 20,
        new_hash: vec![3; 32],
        ..Default::default()
    });
    call.storage_changes.push(write(AUSDT, [99; 32], 0, 1, 25));
    block.transaction_traces.push(tx(call));
    assert_eq!(project(&block, &config).unwrap().holder_basis[0].epoch, 3);
    // Even skipped old-layout effects must retain physical key continuity.
    block.transaction_traces[0].calls[0].storage_changes.push(write(AUSDT, [99; 32], 8, 9, 26));
    assert!(project(&block, &config).unwrap_err().to_string().contains("discontinuous"));
}
