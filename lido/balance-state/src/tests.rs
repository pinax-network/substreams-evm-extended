use super::*;

const EPOCH: &str = include_str!("../tests/fixtures/mainnet-steth-v4-epoch.json");

fn config() -> Config {
    parse(EPOCH).unwrap()
}
fn epoch() -> Epoch {
    config().epochs[0].clone()
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
fn packed(low: u128, high: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(&high.to_be_bytes());
    out[16..].copy_from_slice(&low.to_be_bytes());
    out
}
fn write(key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: epoch().steth,
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
fn shares_call(holder: &[u8], old: u128, new: u128, ordinal: u64) -> eth::Call {
    let e = epoch();
    eth::Call {
        index: 1,
        address: e.steth.clone(),
        keccak_preimages: [preimage(holder, &e.shares_slot)].into(),
        storage_changes: vec![write(mapping_key(holder, &e.shares_slot), w(old), w(new), ordinal)],
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
fn committed_slots_are_the_keccak_of_their_pinned_names() {
    let e = epoch();
    assert_eq!(e.total_and_external_shares_slot, keccak(b"lido.StETH.totalAndExternalShares"));
    assert_eq!(e.buffered_slot, keccak(b"lido.Lido.bufferedEtherAndDepositedPostReport"));
    assert_eq!(e.cl_slot, keccak(b"lido.Lido.clValidatorsBalanceAndClPendingBalance"));
    assert_eq!(e.contract_version_slot, keccak(b"lido.Versioned.contractVersion"));
    for name in [
        "lido.Lido.lidoLocatorAndMaxExternalRatio",
        "lido.Lido.depositedNextReportAndLastDepositNonce",
        "lido.Lido.seedDepositsCount",
        "lido.Lido.stakeLimit",
        "lido.Lido.totalELRewardsCollected",
        "lido.Lido.depositsReserve",
        "lido.Lido.depositsReserveTarget",
        "lido.Pausable.activeFlag",
        "aragonOS.reentrancyGuard.mutex",
    ] {
        assert!(e.other_slots.contains(&keccak(name.as_bytes())), "{name} not reviewed");
    }
    // The pre-V3 total-shares slot is not part of this epoch: a write to it fails closed.
    assert!(!e.other_slots.contains(&keccak(b"lido.StETH.totalShares")));
    assert_eq!(
        keccak(b"TokenRebased(uint256,uint256,uint256,uint256,uint256,uint256,uint256)"),
        TOKEN_REBASED_TOPIC0
    );
}

#[test]
fn share_writes_are_holder_rows_and_packed_words_split_low_and_high() {
    let e = epoch();
    let cfg = config();
    let mut b = block(10);
    let mut call = shares_call(&[9; 20], 1_000, 1_500, 10);
    call.storage_changes.extend([
        write(e.total_and_external_shares_slot, packed(9_000_000, 100_000), packed(9_000_500, 100_000), 11),
        write(e.buffered_slot, packed(5_000, 32_000), packed(5_600, 32_000), 12),
    ]);
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.holder_basis.len(), 1);
    let h = &events.holder_basis[0];
    assert_eq!(
        (&*h.previous_value, &*h.value, h.basis_kind, h.bit_width),
        ("1000", "1500", pb::BasisKind::Shares as i32, 256)
    );
    assert_eq!(
        fields(&events),
        vec![
            (pb::StateField::LidoTotalShares as i32, "9000000".into(), "9000500".into(), 1),
            // Unchanged halves of a written word are still rows (one row per decoded field).
            (pb::StateField::LidoExternalShares as i32, "100000".into(), "100000".into(), 1),
            (pb::StateField::LidoBufferedEther as i32, "5000".into(), "5600".into(), 1),
            (pb::StateField::LidoDepositedPostReport as i32, "32000".into(), "32000".into(), 1),
        ]
    );
    let total = &events.global_state[0];
    assert_eq!(
        (total.bit_offset, total.bit_width, &total.storage_slot),
        (0, 128, &e.total_and_external_shares_slot.to_vec())
    );
    // No derived pooled ether because the CL word was not written.
    assert!(events.epochs.is_empty());
    assert_eq!(events.clocks[0].global_state_count, 4);
    // Large values keep 128-bit precision.
    let huge = u128::MAX - 1;
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![write(e.cl_slot, packed(0, 0), packed(huge, 3), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events),
        vec![
            (pb::StateField::LidoClValidatorsBalance as i32, "0".into(), huge.to_string(), 1),
            (pb::StateField::LidoClPendingBalance as i32, "0".into(), "3".into(), 1)
        ]
    );
}

#[test]
fn a_report_derives_total_pooled_ether_only_when_every_input_word_was_written() {
    let e = epoch();
    let cfg = config();
    let mut b = block(10);
    // internalEther = 100 + 50 + 30 + 20 = 200; totalShares 1000, external 200 -> internal 800;
    // externalEther = 200 * 200 / 800 = 50; total = 250.
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![
            write(e.total_and_external_shares_slot, packed(990, 200), packed(1000, 200), 10),
            write(e.buffered_slot, packed(90, 50), packed(100, 50), 11),
            write(e.cl_slot, packed(25, 20), packed(30, 20), 12),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    let derived: Vec<_> = events
        .global_state
        .iter()
        .filter(|g| g.observation == pb::Observation::Derived as i32)
        .collect();
    assert_eq!(derived.len(), 1);
    assert_eq!(
        (
            derived[0].field,
            &*derived[0].value,
            derived[0].ordinal,
            derived[0].first_ordinal,
            derived[0].change_count
        ),
        (pb::StateField::LidoTotalPooledEther as i32, "250", 12, 10, 3)
    );
    // Truncation: externalEther = 7 * 200 / 993 = 1 (floor).
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![
            write(e.total_and_external_shares_slot, packed(1000, 200), packed(1000, 7), 10),
            write(e.buffered_slot, packed(90, 50), packed(100, 50), 11),
            write(e.cl_slot, packed(25, 20), packed(30, 20), 12),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        events
            .global_state
            .iter()
            .find(|g| g.field == pb::StateField::LidoTotalPooledEther as i32)
            .unwrap()
            .value,
        "201"
    );
    // Zero internal shares: not derivable, inputs still emitted.
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![
            write(e.total_and_external_shares_slot, packed(1000, 200), packed(5, 5), 10),
            write(e.buffered_slot, packed(90, 50), packed(100, 50), 11),
            write(e.cl_slot, packed(25, 20), packed(30, 20), 12),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert!(events.global_state.iter().all(|g| g.field != pb::StateField::LidoTotalPooledEther as i32));
    assert_eq!(events.global_state.len(), 6);
}

#[test]
fn token_rebased_logs_are_report_evidence_from_succeeded_transactions_only() {
    let e = epoch();
    let cfg = config();
    let mut b = block(10);
    let mut data = Vec::new();
    for v in [3600u128, 900, 1_000_000, 1_000_100, 1_000_500, 100] {
        data.extend_from_slice(&w(v));
    }
    let log = eth::Log {
        address: e.steth.clone(),
        topics: vec![TOKEN_REBASED_TOPIC0.to_vec(), w(1_789_689_600).to_vec()],
        data: data.clone(),
        index: 4,
        block_index: 40,
        ordinal: 77,
    };
    let mut t = tx(eth::Call::default());
    t.receipt = Some(eth::TransactionReceipt {
        logs: vec![log.clone()],
        ..Default::default()
    });
    let mut failed = t.clone();
    failed.status = eth::TransactionTraceStatus::Failed as i32;
    failed.hash = vec![8; 32];
    b.transaction_traces = vec![t, failed];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events),
        vec![
            (pb::StateField::LidoReportTimestamp as i32, "".into(), "1789689600".into(), 2),
            (pb::StateField::LidoReportPostTotalShares as i32, "".into(), "1000100".into(), 2),
            (pb::StateField::LidoReportPostTotalEther as i32, "".into(), "1000500".into(), 2),
            (pb::StateField::LidoReportSharesMintedAsFees as i32, "".into(), "100".into(), 2),
        ]
    );
    assert_eq!(
        (
            events.global_state[0].log_index,
            events.global_state[0].ordinal,
            events.global_state[0].boundary
        ),
        (4, 77, pb::Boundary::Change as i32)
    );
    // A log from another address is ignored; a malformed one fails.
    let mut other = log.clone();
    other.address = vec![5; 20];
    let mut t = tx(eth::Call::default());
    t.receipt = Some(eth::TransactionReceipt {
        logs: vec![other],
        ..Default::default()
    });
    b.transaction_traces = vec![t];
    assert!(project(&b, &cfg).unwrap().global_state.is_empty());
    let mut short = log;
    short.data.truncate(5 * 32);
    let mut t = tx(eth::Call::default());
    t.receipt = Some(eth::TransactionReceipt {
        logs: vec![short],
        ..Default::default()
    });
    b.transaction_traces = vec![t];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("malformed TokenRebased"));
}

#[test]
fn version_and_code_changes_invalidate_and_unknown_writes_fail_closed() {
    let e = epoch();
    let cfg = config();
    let mut b = block(10);
    // Upgrade into the qualified version inside the epoch: the old word 3 shows
    // the epoch was active over v3 storage, so it invalidates like any other
    // version write (the activation ordinal belongs after the migration).
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![write(e.contract_version_slot, w(3), w(4), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((events.global_state.len(), events.epochs.len()), (1, 1));
    assert_eq!(
        (events.epochs[0].reason, &events.epochs[0].evidence_previous_word),
        (pb::InvalidationReason::ContractVersionSet as i32, &w(3).to_vec())
    );
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![write(e.contract_version_slot, w(4), w(5), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (events.epochs[0].kind, events.epochs[0].reason, &events.epochs[0].evidence_word),
        (
            pb::EpochEventKind::Invalidated as i32,
            pb::InvalidationReason::ContractVersionSet as i32,
            &w(5).to_vec()
        )
    );
    for (address, reason) in [
        (e.steth.clone(), pb::InvalidationReason::CodeChange),
        (e.implementation.clone(), pb::InvalidationReason::DependencyCodeChange),
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
        assert_eq!((events.epochs.len(), events.epochs[0].reason), (1, reason as i32));
    }
    // Pre-V3 total shares slot, or any unknown key, refuses the block.
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![write(keccak(b"lido.StETH.totalShares"), w(1), w(0), 10)],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("unresolved"));
    // Reviewed names and allowance / nonce mappings are ignored.
    let owner = mapping_key(&[4; 20], &e.other_mapping_slots[0]);
    let spender = mapping_key(&[5; 20], &owner);
    let mut p2 = vec![0u8; 64];
    p2[12..32].copy_from_slice(&[5; 20]);
    p2[32..].copy_from_slice(&owner);
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        keccak_preimages: [preimage(&[4; 20], &e.other_mapping_slots[0]), (hex::encode(spender), hex::encode(p2))].into(),
        storage_changes: vec![
            write(spender, w(0), w(9), 10),
            write(keccak(b"aragonOS.reentrancyGuard.mutex"), w(0), w(1), 11),
            write(keccak(b"aragonOS.reentrancyGuard.mutex"), w(1), w(0), 12),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty());
    // Reverted frames and ordering faults.
    let mut call = shares_call(&[9; 20], 1, 2, 10);
    call.state_reverted = true;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    let mut call = shares_call(&[9; 20], 1, 2, 10);
    let key = mapping_key(&[9; 20], &e.shares_slot);
    call.storage_changes.push(write(key, w(3), w(4), 11));
    b.transaction_traces = vec![tx(call.clone())];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("discontinuous"));
    call.storage_changes[1] = write(key, w(2), w(4), 10);
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("ambiguous"));
    let mut b = block(10);
    b.ver = 4;
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("producer version"));
}

#[test]
fn activation_binds_the_epoch_and_parameters_fail_closed() {
    let e = epoch();
    let cfg = config();
    let events = project(&block(1), &cfg).unwrap();
    let ep = &events.epochs[0];
    assert_eq!(
        (ep.family, ep.basis_kind, &*ep.implementation_revision, ep.balance_decimals, ep.global_carryover),
        (pb::ModelFamily::LidoSteth as i32, pb::BasisKind::Shares as i32, "4", 18, false)
    );
    // Sorted by the proto key (market, epoch, depth, role, contract).
    let roles: Vec<(u32, i32)> = events.dependencies.iter().map(|d| (d.depth, d.role)).collect();
    assert_eq!(
        roles,
        vec![
            (1, pb::DependencyRole::Beacon as i32),
            (1, pb::DependencyRole::Accounting as i32),
            (2, pb::DependencyRole::Implementation as i32),
            (2, pb::DependencyRole::Implementation as i32),
        ]
    );
    assert_eq!(fields(&events), vec![(pb::StateField::LidoContractVersion as i32, "".into(), "4".into(), 3)]);
    // The declaration names the implementation that binds the constant; no slot.
    let constant = &events.global_state[0];
    assert_eq!((&constant.storage_contract, constant.storage_slot.is_empty()), (&e.implementation, true));
    assert!(project(&block(2), &cfg).unwrap().epochs.is_empty());
    assert_eq!(project(&block(1001), &cfg).unwrap().epochs[0].kind, pb::EpochEventKind::Reaffirmed as i32);
    assert!(parse(r#"{"chain_id":1,"producer_versions":[5],"epochs":[]}"#).unwrap().epochs.is_empty());
    let mutate = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
        f(&mut v);
        parse(&v.to_string()).map(|_| ()).unwrap_err().to_string()
    };
    assert!(mutate(
        &|v| v["epochs"][0]["cl_validators_balance_and_cl_pending_balance_slot"] = v["epochs"][0]["buffered_ether_and_deposited_post_report_slot"].clone()
    )
    .contains("overlap"));
    assert!(mutate(&|v| v["epochs"][0]["other_slot_names"] = serde_json::json!(["lido.Versioned.contractVersion"])).contains("overlap"));
    assert!(mutate(&|v| v["epochs"][0]["contract_version"] = 0.into()).contains("positive"));
    assert!(mutate(&|v| v["epochs"][0]["extra"] = 1.into()).contains("unknown field"));
    assert!(mutate(&|v| v["producer_versions"] = serde_json::json!([])).contains("producer_versions"));
    assert!(mutate(&|v| v["producer_versions"] = serde_json::json!([3])).contains("4 and 5"));
}

#[test]
fn shared_hardening_rules_hold_for_steth() {
    use prost::Message;
    let cfg = config();
    let e = epoch();
    let holder = [9u8; 20];
    let key = mapping_key(&holder, &e.shares_slot);
    // A delegatecall frame (Call.address == implementation) writing the proxy's storage,
    // with the Aragon reentrancy mutex flipped 0->1->0 in the same frame.
    let mutex = keccak(b"aragonOS.reentrancyGuard.mutex");
    let mut b = block(10);
    b.transaction_traces = vec![eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 9,
        calls: vec![
            eth::Call {
                index: 0,
                address: e.steth.clone(),
                ..Default::default()
            },
            eth::Call {
                index: 1,
                parent_index: 0,
                depth: 1,
                call_type: eth::CallType::Delegate as i32,
                address: e.implementation.clone(),
                keccak_preimages: [preimage(&holder, &e.shares_slot)].into(),
                storage_changes: vec![write(mutex, w(0), w(1), 10), write(key, w(1), w(2), 11), write(mutex, w(1), w(0), 12)],
                ..Default::default()
            },
        ],
        ..Default::default()
    }];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (
            events.holder_basis.len(),
            &*events.holder_basis[0].value,
            &events.holder_basis[0].storage_contract
        ),
        (1, "2", &e.steth)
    );
    // FAILED and REVERTED transactions contribute nothing; status 0 is refused.
    for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
        let mut t = tx(shares_call(&holder, 1, 2, 10));
        t.status = status as i32;
        b.transaction_traces = vec![t];
        assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    }
    let mut t = tx(eth::Call::default());
    t.status = 0;
    b.transaction_traces = vec![t];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("incomplete transaction"));
    // Blocks before activation emit only the clock.
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    v["epochs"][0]["activation_block"] = 100.into();
    let late = parse(&v.to_string()).unwrap();
    let mut b = block(50);
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![write(w(0x77), w(0), w(1), 10)],
        ..Default::default()
    })];
    let events = project(&b, &late).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty() && events.epochs.is_empty());
    assert_eq!(events.clocks.len(), 1);
    // Ordering faults name the contract, key and ordinals.
    let mut call = shares_call(&holder, 1, 2, 10);
    call.storage_changes.push(write(key, w(3), w(4), 11));
    let mut b = block(10);
    b.transaction_traces = vec![tx(call)];
    let err = project(&b, &cfg).unwrap_err().to_string();
    assert!(err.contains("discontinuous") && err.contains(&hex::encode(&e.steth)) && err.contains(&hex::encode(key)) && err.contains("ordinal 11"));
    // Determinism under input permutation.
    let mut b = block(10);
    let mut second = tx(shares_call(&[8; 20], 5, 6, 12));
    second.index = 10;
    second.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&holder, 1, 2, 10)), second];
    let forward = project(&b, &cfg).unwrap();
    let mut reversed = b.clone();
    reversed.transaction_traces.reverse();
    assert_eq!(forward.encode_to_vec(), project(&reversed, &cfg).unwrap().encode_to_vec());
}

type ResolutionTarget = (Vec<u8>, [u8; 32], [u8; 32], pb::InvalidationReason);

fn resolution_targets(e: &Epoch) -> Vec<ResolutionTarget> {
    vec![
        (
            e.steth.clone(),
            e.aragon.kernel_slot,
            word(&e.aragon.kernel).unwrap(),
            pb::InvalidationReason::ImplementationPointerWrite,
        ),
        (
            e.steth.clone(),
            e.aragon.app_id_slot,
            e.aragon.app_id,
            pb::InvalidationReason::ImplementationPointerWrite,
        ),
        (
            e.aragon.kernel.clone(),
            e.aragon.app_base_slot,
            word(&e.implementation).unwrap(),
            pb::InvalidationReason::DependencyPointerWrite,
        ),
        (
            e.aragon.kernel.clone(),
            e.aragon.kernel_implementation_slot,
            word(&e.aragon.kernel_implementation).unwrap(),
            pb::InvalidationReason::DependencyPointerWrite,
        ),
    ]
}

fn at(address: &[u8], key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        ..write(key, old, new, ordinal)
    }
}

#[test]
fn aragon_resolution_is_explicitly_bound_and_cannot_be_ignored_in_parameters() {
    let cfg = config();
    let e = &cfg.epochs[0];
    assert_eq!(
        hex::encode(e.aragon.kernel_slot),
        "4172f0f7d2289153072b0a6ca36959e0cbe2efc3afe50fc81636caa96338137b"
    );
    assert_eq!(
        hex::encode(e.aragon.app_id_slot),
        "d625496217aa6a3453eecb9c3489dc5a53e6c67b444329ea2b2cbc9ff547639b"
    );
    assert_eq!(hex::encode(keccak(b"base")), "f1f3eb40f5bc1ad1344716ced8b8a0431d840b5783aea1fd01786bc26f35ac0f");
    assert_eq!(hex::encode(keccak(b"core")), "c681a85306374a5ab27f0bbc385296a54bcd314a1948b6cf61c4ea1bc44bb9f8");
    for (number, kind) in [(1, pb::EpochEventKind::Bound), (1001, pb::EpochEventKind::Reaffirmed)] {
        let events = project(&block(number), &cfg).unwrap();
        for (address, key, expected, _) in resolution_targets(e).into_iter().filter(|(_, key, _, _)| *key != e.aragon.app_id_slot) {
            let edge = events
                .dependencies
                .iter()
                .find(|d| d.pointer_contract == address && d.pointer_slot == key)
                .unwrap();
            assert_eq!(edge.kind, kind as i32);
            assert_eq!(edge.binding, pb::BindingKind::StoragePointer as i32);
            assert_eq!(edge.pointer_value, expected);
            assert_eq!(edge.contract, expected[12..]);
            assert_eq!(edge.source_pin, format!("{}; {}", e.source_pin, e.aragon.source_pin));
            assert!(edge.code_hash.is_empty()); // Source bindings are not runtime qualification.
            if address == e.steth {
                assert_eq!((edge.role, edge.depth), (pb::DependencyRole::Beacon as i32, 1));
                assert!(edge.parent.is_empty());
            } else {
                assert_eq!((edge.role, edge.depth), (pb::DependencyRole::Implementation as i32, 2));
                assert_eq!(edge.parent, e.aragon.kernel);
            }
        }
        assert_eq!(events.clocks[0].dependency_count, 4);
    }
    // Kernel/apps are configured guard slots, never a caller's ignored slots.
    for name in ["aragonOS.appStorage.kernel", "aragonOS.appStorage.appId"] {
        let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
        v["epochs"][0]["other_slot_names"].as_array_mut().unwrap().push(name.into());
        assert!(parse(&v.to_string()).unwrap_err().to_string().contains("overlap"));
    }
    let mut missing: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    missing["epochs"][0].as_object_mut().unwrap().remove("aragon");
    assert!(parse(&missing.to_string()).unwrap_err().to_string().contains("aragon"));
    for (field, value, expected) in [
        ("source_pin", "", "pinned OS 4.4.0"),
        ("source_pin", "aragon/aragonOS@unreviewed", "pinned OS 4.4.0"),
        ("kernel", "0x0000000000000000000000000000000000000000", "nonzero"),
        ("kernel_implementation", "0x01", "20 bytes"),
        ("app_id", "0x0000000000000000000000000000000000000000000000000000000000000000", "nonzero"),
    ] {
        let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
        v["epochs"][0]["aragon"][field] = value.into();
        assert!(parse(&v.to_string()).unwrap_err().to_string().contains(expected));
    }
}

#[test]
fn every_resolution_write_invalidates_even_when_restored_or_rebound_to_expected() {
    let cfg = config();
    let e = &cfg.epochs[0];
    for (address, key, expected, reason) in resolution_targets(e) {
        let mut b = block(10);
        // Restoration still invalidates, and each transition retains its own provenance.
        let mut restore = tx(eth::Call {
            index: 4,
            address: e.aragon.kernel_implementation.clone(),
            storage_changes: vec![at(&address, key, w(7), expected, 20)],
            ..Default::default()
        });
        restore.hash = vec![8; 32];
        restore.index = 10;
        b.transaction_traces = vec![
            tx(eth::Call {
                index: 3,
                address: e.implementation.clone(),
                storage_changes: vec![at(&address, key, expected, w(7), 10)],
                ..Default::default()
            }),
            restore.clone(),
        ];
        let events = project(&b, &cfg).unwrap();
        assert_eq!(events.epochs.len(), 2);
        assert!(events.holder_basis.is_empty() && events.global_state.is_empty());
        for (i, row) in events.epochs.iter().enumerate() {
            assert_eq!(row.kind, pb::EpochEventKind::Invalidated as i32);
            assert_eq!(row.reason, reason as i32);
            assert_eq!(row.evidence_contract, address);
            assert_eq!(row.evidence_slot, key);
            assert_eq!(row.ordinal, if i == 0 { 10 } else { 20 });
            assert_eq!(row.evidence_previous_word, if i == 0 { expected } else { w(7) });
            assert_eq!(row.evidence_word, if i == 0 { w(7) } else { expected });
            assert_eq!(row.transaction_index, 9 + i as u32);
            assert_eq!(row.call_index, 3 + i as u32);
            assert_eq!(row.transaction_hash, vec![7 + i as u8; 32]);
        }
        // A write into the configured identity is not proof the preceding era was qualified.
        b.transaction_traces = vec![restore];
        assert_eq!(project(&b, &cfg).unwrap().epochs.len(), 1);
        // STORAGE_POINTER binds every persisted write, including equal-value SSTOREs.
        b.transaction_traces[0].calls[0].storage_changes[0].old_value = expected.to_vec();
        let events = project(&b, &cfg).unwrap();
        assert_eq!(events.epochs.len(), 1);
        assert_eq!(events.epochs[0].evidence_previous_word, expected);
        assert_eq!(events.epochs[0].evidence_word, expected);
    }
    // Contract-version excursions must not disappear in end-of-block reduction either.
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        storage_changes: vec![write(e.contract_version_slot, w(4), w(5), 10), write(e.contract_version_slot, w(5), w(4), 11)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.global_state[0].value, "4");
    // Both writes are evidence: the excursion 4 -> 5 -> 4 is not hidden.
    let evidence: Vec<(u64, i32, Vec<u8>)> = events.epochs.iter().map(|x| (x.ordinal, x.reason, x.evidence_word.clone())).collect();
    assert_eq!(
        evidence,
        vec![
            (10, pb::InvalidationReason::ContractVersionSet as i32, w(5).to_vec()),
            (11, pb::InvalidationReason::ContractVersionSet as i32, w(4).to_vec()),
        ]
    );
}

#[test]
fn resolution_guards_follow_persistence_and_refuse_bad_ordering() {
    let cfg = config();
    let e = &cfg.epochs[0];
    for (address, key, expected, _) in resolution_targets(e) {
        let call = eth::Call {
            address: e.aragon.kernel_implementation.clone(),
            storage_changes: vec![at(&address, key, expected, w(7), 10)],
            ..Default::default()
        };
        let mut b = block(10);
        for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
            let mut failed = tx(call.clone());
            failed.status = status as i32;
            b.transaction_traces = vec![failed];
            assert!(project(&b, &cfg).unwrap().epochs.is_empty());
        }
        let mut reverted = call.clone();
        reverted.state_reverted = true;
        b.transaction_traces = vec![tx(reverted)];
        assert!(project(&b, &cfg).unwrap().epochs.is_empty());
        b.transaction_traces.clear();
        b.system_calls = vec![call.clone()];
        assert_eq!(project(&b, &cfg).unwrap().epochs[0].scope, pb::Scope::SystemCall as i32);
        b.system_calls.clear();
        for (second_old, ordinal, message) in [(w(7), 10, "ambiguous"), (w(8), 20, "discontinuous")] {
            let mut bad = call.clone();
            bad.storage_changes.push(at(&address, key, second_old, expected, ordinal));
            b.transaction_traces = vec![tx(bad)];
            let err = project(&b, &cfg).unwrap_err().to_string();
            assert!(err.contains(message) && err.contains(&hex::encode(&address)) && err.contains(&hex::encode(key)));
        }
    }
    // Other app bases and ordinary Kernel/implementation/accounting state are not conversion inputs.
    let other_app_base = mapping_key(&w(99), &mapping_key(&keccak(b"base"), &[0; 32]));
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        storage_changes: vec![
            at(&e.aragon.kernel, other_app_base, w(1), w(2), 10),
            at(&e.aragon.kernel, w(1), w(1), w(2), 11),
            at(&e.aragon.kernel_implementation, w(0), w(1), w(2), 12),
            at(e.accounting.as_ref().unwrap(), w(0), w(1), w(2), 13),
        ],
        ..Default::default()
    })];
    assert!(project(&b, &cfg).unwrap().epochs.is_empty());
}

#[test]
fn dependency_code_changes_invalidate_with_deterministic_evidence() {
    use prost::Message;
    let cfg = config();
    let e = &cfg.epochs[0];
    let mut b = block(10);
    let mut codes = vec![];
    for address in [
        &e.steth,
        &e.implementation,
        &e.aragon.kernel,
        &e.aragon.kernel_implementation,
        e.accounting.as_ref().unwrap(),
    ] {
        let call = eth::Call {
            code_changes: vec![eth::CodeChange {
                address: address.clone(),
                old_hash: vec![1; 32],
                new_hash: vec![2; 32],
                ordinal: 10,
                ..Default::default()
            }],
            ..Default::default()
        };
        b.transaction_traces = vec![tx(call.clone())];
        let events = project(&b, &cfg).unwrap();
        let reason = if *address == e.steth {
            pb::InvalidationReason::CodeChange
        } else {
            pb::InvalidationReason::DependencyCodeChange
        };
        assert_eq!(events.epochs.len(), 1);
        assert_eq!(events.epochs[0].reason, reason as i32);
        assert_eq!(events.epochs[0].evidence_contract, *address);
        assert_eq!(events.epochs[0].evidence_code_hash, vec![2; 32]);
        for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
            b.transaction_traces[0].status = status as i32;
            assert!(project(&b, &cfg).unwrap().epochs.is_empty());
        }
        b.transaction_traces = vec![tx(eth::Call {
            state_reverted: true,
            ..call.clone()
        })];
        assert!(project(&b, &cfg).unwrap().epochs.is_empty());
        b.transaction_traces.clear();
        b.system_calls = vec![call.clone()];
        assert_eq!(project(&b, &cfg).unwrap().epochs[0].scope, pb::Scope::SystemCall as i32);
        b.system_calls.clear();
        codes.extend(call.code_changes);
    }
    // Block-level dependency changes also persist, and equal cross-contract ordinals have stable ordering.
    b.transaction_traces.clear();
    b.code_changes = codes;
    let original = project(&b, &cfg).unwrap();
    assert_eq!(original.epochs.len(), 5);
    assert!(original.epochs.iter().all(|e| e.scope == pb::Scope::Block as i32));
    b.code_changes.reverse();
    assert_eq!(original.encode_to_vec(), project(&b, &cfg).unwrap().encode_to_vec());
}

#[test]
fn shared_kernel_guards_are_attributed_to_every_affected_market() {
    use prost::Message;
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    let mut second = v["epochs"][0].clone();
    second["steth"] = format!("0x{}", hex::encode([0x22; 20])).into();
    second["aragon"]["app_id"] = format!("0x{}", hex::encode(w(99))).into();
    v["epochs"].as_array_mut().unwrap().push(second);
    let cfg = parse(&v.to_string()).unwrap();
    let first = &cfg.epochs[0];
    let second = &cfg.epochs[1];
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        storage_changes: vec![at(
            &first.aragon.kernel,
            first.aragon.app_base_slot,
            word(&first.implementation).unwrap(),
            w(9),
            10,
        )],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 1);
    assert_eq!(events.epochs[0].market, first.steth);
    b.transaction_traces.push(tx(eth::Call {
        storage_changes: vec![
            at(
                &first.aragon.kernel,
                first.aragon.kernel_implementation_slot,
                word(&first.aragon.kernel_implementation).unwrap(),
                w(8),
                20,
            ),
            at(
                &first.aragon.kernel,
                first.aragon.kernel_implementation_slot,
                w(8),
                word(&first.aragon.kernel_implementation).unwrap(),
                30,
            ),
            at(
                &second.aragon.kernel,
                second.aragon.app_base_slot,
                word(&second.implementation).unwrap(),
                w(7),
                40,
            ),
        ],
        ..Default::default()
    }));
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 6);
    assert_eq!(events.epochs.iter().filter(|e| e.market == first.steth).count(), 3);
    assert_eq!(events.epochs.iter().filter(|e| e.market == second.steth).count(), 3);
    b.transaction_traces.reverse();
    for t in &mut b.transaction_traces {
        t.calls[0].storage_changes.reverse();
    }
    assert_eq!(events.encode_to_vec(), project(&b, &cfg).unwrap().encode_to_vec());
}

#[test]
fn validate_block_refusals_provenance_and_multi_epoch_attribution() {
    let cfg = config();
    let e = epoch();
    type Mutation = Box<dyn Fn(&mut eth::Block)>;
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "Extended blocks required",
            Box::new(|b| b.detail_level = eth::block::DetailLevel::DetaillevelBase as i32),
        ),
        ("producer version", Box::new(|b| b.ver = 3)),
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
    // Same-block repeated writes keep the first old value and the last write's provenance.
    let holder = [9u8; 20];
    let mut b = block(10);
    let mut second = tx(shares_call(&holder, 2, 7, 20));
    second.index = 10;
    second.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&holder, 1, 2, 10)), second];
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
    // Two stETH epochs (a second deployment address) written in one block are attributed by address.
    let other: Vec<u8> = vec![0x22; 20];
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    let mut second_epoch = v["epochs"][0].clone();
    second_epoch["steth"] = format!("0x{}", hex::encode(&other)).into();
    second_epoch["implementation"] = "0x0000000000000000000000000000000000000002".into();
    v["epochs"].as_array_mut().unwrap().push(second_epoch);
    let two = parse(&v.to_string()).unwrap();
    let key = mapping_key(&holder, &e.shares_slot);
    let mut other_tx = tx(eth::Call {
        index: 1,
        address: other.clone(),
        keccak_preimages: [preimage(&holder, &e.shares_slot)].into(),
        storage_changes: vec![eth::StorageChange {
            address: other.clone(),
            key: key.to_vec(),
            old_value: w(0).to_vec(),
            new_value: w(9).to_vec(),
            ordinal: 12,
        }],
        ..Default::default()
    });
    other_tx.index = 10;
    other_tx.hash = vec![8; 32];
    let mut b = block(10);
    b.transaction_traces = vec![tx(shares_call(&holder, 1, 2, 10)), other_tx];
    let events = project(&b, &two).unwrap();
    let rows: Vec<(&Vec<u8>, &str)> = events.holder_basis.iter().map(|h| (&h.market, h.value.as_str())).collect();
    assert_eq!(rows, vec![(&other, "9"), (&e.steth, "2")]);
    assert_eq!(events.clocks[0].holder_basis_count, 2);
}

#[test]
fn an_epoch_bound_mid_block_owns_only_effects_from_its_activation_ordinal() {
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    v["epochs"][0]["activation_block"] = 10.into();
    v["epochs"][0]["activation_ordinal"] = 100.into();
    let cfg = parse(&v.to_string()).unwrap();
    let e = cfg.epochs[0].clone();
    let holder = [9u8; 20];
    let key = mapping_key(&holder, &e.shares_slot);
    // The Kernel app-base write that installs this epoch's implementation
    // (ordinal 50) belongs to the previous epoch and must not invalidate this one.
    let mut implementation_word = [0u8; 32];
    implementation_word[12..].copy_from_slice(&e.implementation);
    let kernel_write = eth::StorageChange {
        address: e.aragon.kernel.clone(),
        key: e.aragon.app_base_slot.to_vec(),
        old_value: w(0xdead).to_vec(),
        new_value: implementation_word.to_vec(),
        ordinal: 50,
    };
    let rebased = |ordinal: u64| {
        let mut data = Vec::new();
        for value in [3600u128, 900, 1_000_000, 1_000_100, 1_000_500, 100] {
            data.extend_from_slice(&w(value));
        }
        eth::Log {
            address: e.steth.clone(),
            topics: vec![TOKEN_REBASED_TOPIC0.to_vec(), w(1_789_689_600).to_vec()],
            data,
            index: 0,
            block_index: 0,
            ordinal,
        }
    };
    let mut call = shares_call(&holder, 1, 2, 60);
    call.storage_changes.push(write(key, w(2), w(9), 150));
    let mut kernel_call = eth::Call {
        index: 2,
        address: e.aragon.kernel.clone(),
        storage_changes: vec![kernel_write],
        ..Default::default()
    };
    kernel_call.logs = vec![];
    let mut t = tx(call);
    t.calls.push(kernel_call);
    t.receipt = Some(eth::TransactionReceipt {
        logs: vec![rebased(70)],
        ..Default::default()
    });
    let mut b = block(10);
    b.transaction_traces = vec![t];
    let events = project(&b, &cfg).unwrap();
    assert!(events.epochs.iter().all(|ep| ep.kind != pb::EpochEventKind::Invalidated as i32));
    // The report log at ordinal 70 predates the epoch and is not its evidence.
    assert!(events.global_state.iter().all(|g| g.observation != pb::Observation::ObservedLog as i32));
    assert_eq!(events.holder_basis.len(), 1);
    assert_eq!(
        (
            &*events.holder_basis[0].previous_value,
            &*events.holder_basis[0].value,
            events.holder_basis[0].first_ordinal
        ),
        ("2", "9", 150)
    );
    let bound = events.epochs.iter().find(|ep| ep.kind == pb::EpochEventKind::Bound as i32).unwrap();
    assert_eq!((bound.activation_ordinal, bound.ordinal), (100, 100));
    // After the activation ordinal the same Kernel write invalidates.
    let mut late = eth::Call {
        index: 1,
        address: e.aragon.kernel.clone(),
        storage_changes: vec![eth::StorageChange {
            address: e.aragon.kernel.clone(),
            key: e.aragon.app_base_slot.to_vec(),
            old_value: implementation_word.to_vec(),
            new_value: w(0xbad).to_vec(),
            ordinal: 120,
        }],
        ..Default::default()
    };
    late.logs = vec![];
    let mut b = block(10);
    b.transaction_traces = vec![tx(late)];
    let invalidated: Vec<(i32, u64)> = project(&b, &cfg)
        .unwrap()
        .epochs
        .iter()
        .filter(|ep| ep.kind == pb::EpochEventKind::Invalidated as i32)
        .map(|ep| (ep.reason, ep.ordinal))
        .collect();
    assert_eq!(invalidated, vec![(pb::InvalidationReason::DependencyPointerWrite as i32, 120)]);
}

fn rebased_log(topics: usize, ordinal: u64) -> eth::Log {
    let mut data = Vec::new();
    for v in [86_400u128, 10_000, 9_800, 10_010, 10_300, 20] {
        data.extend_from_slice(&w(v));
    }
    let mut all = vec![TOKEN_REBASED_TOPIC0.to_vec(), w(1_789_600_000).to_vec(), w(1).to_vec()];
    all.truncate(topics);
    eth::Log {
        address: epoch().steth,
        topics: all,
        data,
        index: 0,
        block_index: 5,
        ordinal,
    }
}
fn delegate(index: u32, preimages: Vec<(String, String)>, changes: Vec<eth::StorageChange>) -> eth::Call {
    let e = epoch();
    eth::Call {
        index,
        depth: 1,
        call_type: eth::CallType::Delegate as i32,
        caller: e.steth.clone(),
        address: e.implementation.clone(),
        keccak_preimages: preimages.into_iter().collect(),
        storage_changes: changes,
        ..Default::default()
    }
}

#[test]
fn the_v3_to_v4_migration_inside_an_epoch_invalidates_with_evidence_instead_of_halting() {
    let e = epoch();
    let cfg = config();
    // `_migrateStorage_v3_to_v4` (Lido.sol:311-341) wipes these function-local positions.
    let retired: Vec<[u8; 32]> = RETIRED_V3_POSITION_NAMES.iter().map(|n| keccak(n.as_bytes())).collect();
    assert_eq!(
        retired.iter().map(hex::encode).collect::<Vec<_>>(),
        vec![
            "c36804a03ec742b57b141e4e5d8d3bd1ddb08451fd0f9983af8aaab357a78e2f",
            "a84c096ee27e195f25d7b6c7c2a03229e49f1a2a5087e57ce7d7127707942fe3"
        ]
    );
    let mut b = block(10);
    b.transaction_traces = vec![tx(delegate(
        1,
        vec![],
        vec![
            write(e.contract_version_slot, w(3), w(4), 10),
            write(e.buffered_slot, packed(0, 0), packed(1_000, 64), 11),
            write(e.cl_slot, packed(0, 0), packed(9_000, 0), 12),
            write(retired[0], packed(9_000, 30), w(0), 13),
            write(retired[1], packed(1_000, 32), w(0), 14),
        ],
    ))];
    let events = project(&b, &cfg).unwrap();
    let evidence: Vec<(u64, i32)> = events.epochs.iter().map(|x| (x.ordinal, x.reason)).collect();
    assert_eq!(
        evidence,
        vec![
            (10, pb::InvalidationReason::ContractVersionSet as i32),
            (13, pb::InvalidationReason::StorageMigration as i32),
            (14, pb::InvalidationReason::StorageMigration as i32),
        ]
    );
    assert_eq!(events.epochs[1].evidence_slot, retired[0].to_vec());
    // Activated after the migration, the same writes are the previous epoch's.
    let mut v: serde_json::Value = serde_json::from_str(EPOCH).unwrap();
    v["epochs"][0]["activation_block"] = 10.into();
    v["epochs"][0]["activation_ordinal"] = 15.into();
    let late = parse(&v.to_string()).unwrap();
    let events = project(&b, &late).unwrap();
    assert!(events.epochs.iter().all(|x| x.kind == pb::EpochEventKind::Bound as i32));
    assert!(events.global_state.iter().all(|g| g.observation == pb::Observation::QualifiedConstant as i32));
}

#[test]
fn the_kernel_resolution_slots_are_the_literal_aragon_mapping_members() {
    // KernelStorage `apps` (slot 0): apps[keccak("base")][appId] and
    // apps[keccak("core")][KERNEL_CORE_APP_ID]; computed independently of the
    // crate (the review's Python keccak) and pinned so an argument swap fails.
    let e = epoch();
    assert_eq!(
        hex::encode(e.aragon.app_base_slot),
        "54b2b2de1ae6731a04bdbca30cee71852851cfcd3298aaf29f4ebff9452b27ad"
    );
    assert_eq!(
        hex::encode(e.aragon.kernel_implementation_slot),
        "8e2ed18767e9c33b25344c240cdf92034fae56be99e2c07f3d9946d949ffede4"
    );
}

#[test]
fn a_routine_report_submit_permit_and_external_mint_block_is_fully_accounted() {
    use prost::Message;
    let e = epoch();
    let cfg = config();
    let named = |n: &str| keccak(n.as_bytes());
    let (wq, burner, treasury, user, spender) = ([0x55u8; 20], [0x66u8; 20], [0x77u8; 20], [0x44u8; 20], [0x33u8; 20]);
    let share = |h: &[u8; 20]| mapping_key(h, &e.shares_slot);
    let shares_preimage = |h: &[u8; 20]| preimage(h, &e.shares_slot);
    // Oracle report: processClStateUpdate, receiveELRewards,
    // collectRewardsAndProcessWithdrawals, the withdrawal queue's burn
    // transfer, fee minting, burnShares, then TokenRebased.
    let mut report = tx(eth::Call::default());
    report.calls = vec![
        eth::Call {
            index: 0,
            address: vec![0xac; 20],
            ..Default::default()
        },
        delegate(
            1,
            vec![],
            vec![
                write(named("lido.Lido.depositedNextReportAndLastDepositNonce"), packed(5, 100), packed(0, 101), 10),
                write(e.buffered_slot, packed(1_000, 64), packed(1_000, 0), 11),
                write(e.cl_slot, packed(9_000, 32), packed(9_100, 0), 12),
            ],
        ),
        delegate(2, vec![], vec![write(named("lido.Lido.totalELRewardsCollected"), w(7), w(9), 20)]),
        delegate(
            3,
            vec![],
            vec![
                write(e.buffered_slot, packed(1_000, 0), packed(1_002, 0), 30),
                write(named("lido.Lido.depositsReserve"), w(0), w(50), 31),
            ],
        ),
        delegate(
            4,
            vec![shares_preimage(&wq), shares_preimage(&burner)],
            vec![write(share(&wq), w(40), w(30), 40), write(share(&burner), w(0), w(10), 41)],
        ),
        delegate(
            5,
            vec![shares_preimage(&treasury)],
            vec![
                write(e.total_and_external_shares_slot, packed(10_000, 500), packed(10_020, 500), 50),
                write(share(&treasury), w(1), w(21), 51),
            ],
        ),
        delegate(
            6,
            vec![shares_preimage(&burner)],
            vec![
                write(e.total_and_external_shares_slot, packed(10_020, 500), packed(10_010, 500), 60),
                write(share(&burner), w(10), w(0), 61),
            ],
        ),
    ];
    report.receipt = Some(eth::TransactionReceipt {
        logs: vec![rebased_log(2, 70)],
        ..Default::default()
    });
    // submit (stake limit), permit (nonces, a two-level allowance), and an
    // external mint writing one packed word twice in one frame.
    let mut submit = tx(delegate(
        1,
        vec![shares_preimage(&user)],
        vec![
            write(named("lido.Lido.stakeLimit"), w(1), w(2), 100),
            write(e.total_and_external_shares_slot, packed(10_010, 500), packed(10_011, 500), 101),
            write(share(&user), w(0), w(1), 102),
            write(e.buffered_slot, packed(1_002, 0), packed(1_003, 0), 103),
        ],
    ));
    submit.index = 10;
    submit.hash = vec![8; 32];
    let (allowances, nonces) = (e.other_mapping_slots[0], e.other_mapping_slots[1]);
    let inner = mapping_key(&user, &allowances);
    let allowance = mapping_key(&spender, &inner);
    let mut permit = tx(delegate(
        1,
        vec![preimage(&user, &nonces), preimage(&user, &allowances), preimage(&spender, &inner)],
        vec![write(mapping_key(&user, &nonces), w(0), w(1), 200), write(allowance, w(0), w(5), 201)],
    ));
    permit.index = 11;
    permit.hash = vec![9; 32];
    let mut external = tx(delegate(
        1,
        vec![shares_preimage(&user)],
        vec![
            write(e.total_and_external_shares_slot, packed(10_011, 500), packed(10_011, 600), 300),
            write(e.total_and_external_shares_slot, packed(10_011, 600), packed(10_111, 600), 301),
            write(share(&user), w(1), w(101), 302),
        ],
    ));
    external.index = 12;
    external.hash = vec![10; 32];
    let mut b = block(10);
    b.transaction_traces = vec![report, submit, permit, external];
    let events = project(&b, &cfg).unwrap();
    let holders: Vec<([u8; 1], String, String)> = events
        .holder_basis
        .iter()
        .map(|h| ([h.holder[0]], h.previous_value.clone(), h.value.clone()))
        .collect();
    assert_eq!(
        holders,
        vec![
            ([0x44], "0".into(), "101".into()),
            ([0x55], "40".into(), "30".into()),
            // Written twice (0 -> 10 -> 0): still an end-of-block row.
            ([0x66], "0".into(), "0".into()),
            ([0x77], "1".into(), "21".into()),
        ]
    );
    let global = |field: pb::StateField| {
        events
            .global_state
            .iter()
            .filter(|g| g.field == field as i32)
            .map(|g| (g.previous_value.clone(), g.value.clone(), g.change_count))
            .collect::<Vec<_>>()
    };
    assert_eq!(global(pb::StateField::LidoTotalShares), vec![("10000".into(), "10111".into(), 5)]);
    assert_eq!(global(pb::StateField::LidoExternalShares), vec![("500".into(), "600".into(), 5)]);
    assert_eq!(global(pb::StateField::LidoBufferedEther), vec![("1000".into(), "1003".into(), 3)]);
    assert_eq!(global(pb::StateField::LidoClValidatorsBalance), vec![("9000".into(), "9100".into(), 1)]);
    // internalEther 1003 + 9100 = 10103 over internalShares 9511, plus
    // 600 × 10103 / 9511 external ether: all three words were written.
    let derived = global(pb::StateField::LidoTotalPooledEther);
    assert_eq!(derived[0].1, (10_103 + 600 * 10_103 / 9_511).to_string());
    assert_eq!(global(pb::StateField::LidoReportPostTotalEther), vec![("".into(), "10300".into(), 1)]);
    assert!(events.epochs.is_empty());
    let mut reversed = b.clone();
    reversed.transaction_traces.reverse();
    assert_eq!(events.encode_to_vec(), project(&reversed, &cfg).unwrap().encode_to_vec());
}

#[test]
fn packed_halves_decode_at_their_full_uint128_width() {
    let e = epoch();
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        storage_changes: vec![write(e.cl_slot, packed(0, 0), packed(u128::MAX, u128::MAX), 10)],
        ..Default::default()
    })];
    let events = project(&b, &config()).unwrap();
    let values: Vec<(i32, String, u32, u32)> = events
        .global_state
        .iter()
        .map(|g| (g.field, g.value.clone(), g.bit_offset, g.bit_width))
        .collect();
    assert_eq!(
        values,
        vec![
            (pb::StateField::LidoClValidatorsBalance as i32, u128::MAX.to_string(), 0, 128),
            (pb::StateField::LidoClPendingBalance as i32, u128::MAX.to_string(), 128, 128),
        ]
    );
}

#[test]
fn report_logs_need_exactly_two_topics_and_a_receipt() {
    let cfg = config();
    for topics in [1, 3] {
        let mut t = tx(eth::Call::default());
        t.receipt = Some(eth::TransactionReceipt {
            logs: vec![rebased_log(topics, 70)],
            ..Default::default()
        });
        let mut b = block(10);
        b.transaction_traces = vec![t];
        assert!(project(&b, &cfg).unwrap_err().to_string().contains("malformed TokenRebased"), "{topics} topics");
    }
    // A succeeded transaction that logged from stETH but carries no receipt
    // is incomplete data; logs of a reverted frame alone do not need one.
    let mut call = eth::Call {
        logs: vec![rebased_log(2, 70)],
        ..Default::default()
    };
    let mut b = block(10);
    b.transaction_traces = vec![tx(call.clone())];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("no receipt"));
    call.state_reverted = true;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap().global_state.is_empty());
}
