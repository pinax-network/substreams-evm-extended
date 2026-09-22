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
    // Upgrade into the qualified version: a row, no invalidation (no-op writes are not persisted).
    b.transaction_traces = vec![tx(eth::Call {
        address: e.steth.clone(),
        storage_changes: vec![write(e.contract_version_slot, w(3), w(4), 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((events.global_state.len(), events.epochs.len()), (1, 0));
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
    for address in [e.steth.clone(), e.implementation.clone()] {
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
        assert_eq!((events.epochs.len(), events.epochs[0].reason), (1, pb::InvalidationReason::CodeChange as i32));
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
    let roles: Vec<i32> = events.dependencies.iter().map(|d| d.role).collect();
    assert_eq!(roles, vec![pb::DependencyRole::Implementation as i32, pb::DependencyRole::Accounting as i32]);
    assert_eq!(fields(&events), vec![(pb::StateField::LidoContractVersion as i32, "".into(), "4".into(), 3)]);
    assert_eq!(events.global_state[0].storage_slot, e.contract_version_slot.to_vec());
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
