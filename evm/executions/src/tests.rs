use super::*;
use prost::Message;

const FULL_BLOCK: &[u8] = include_bytes!("../../../erc20/balances/tests/fixtures/bsc-122260950.pb");
const CLONE_CREATION: &[u8] = include_bytes!("../../../erc20/balances/tests/fixtures/bsc-122288172-clone-creation-tx.pb");
const DEPLOYMENT: &[u8] = include_bytes!("../../../erc20/balances/tests/fixtures/bsc-122288338-deployment-tx.pb");
const FAILED_SETCODE: [&[u8]; 2] = [
    include_bytes!("../../../erc20/balances/tests/fixtures/bsc-121114122-failed-setcode.pb"),
    include_bytes!("../../../erc20/balances/tests/fixtures/bsc-121114153-failed-setcode.pb"),
];

fn config() -> Config {
    parse(r#"{"chain_id":56,"producer_versions":[5]}"#).unwrap()
}
fn block() -> eth::Block {
    eth::Block {
        ver: 5,
        number: 121114122,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 121114122,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp { seconds: 1789000000, nanos: 0 }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn tx(calls: Vec<eth::Call>) -> eth::TransactionTrace {
    eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 3,
        calls,
        receipt: Some(eth::TransactionReceipt::default()),
        ..Default::default()
    }
}
fn log(address: u8, topics: usize, ordinal: u64) -> eth::Log {
    eth::Log {
        address: vec![address; 20],
        topics: (0..topics).map(|i| vec![i as u8; 32]).collect(),
        data: vec![9; 64],
        index: 0,
        block_index: 0,
        ordinal,
    }
}

#[test]
fn captured_block_keeps_attempted_and_persisted_execution_apart() {
    let block = eth::Block::decode(FULL_BLOCK).unwrap();
    let events = project(&block, &config()).unwrap();
    let clock = &events.clocks[0];
    assert_eq!(
        (clock.number, clock.producer_version, clock.transaction_count, clock.system_call_count),
        (122260950, 5, 68, 1)
    );
    assert_eq!((clock.call_count, clock.log_count, clock.code_change_count), (853, 273, 1));
    assert_eq!(
        format!("0x{}", hex::encode(&clock.hash)),
        "0x00de5f67359c37d39e58dca906906eb8b72b3bc2fce3a087746a3acb95fcbab4"
    );
    assert!(!clock.coinbase.is_empty() && clock.gas_used > 0);
    // Persisted logs are exactly the receipt logs; reverted frames' logs stay
    // visible as attempts.
    let persisted_logs = events.logs.iter().filter(|l| l.persisted).count() as u32;
    assert_eq!(persisted_logs, events.transactions.iter().map(|t| t.receipt_log_count).sum::<u32>());
    assert_eq!((persisted_logs, events.logs.len()), (253, 273));
    // Attempted logs keep a frame-local index but no block index.
    assert!(events.logs.iter().filter(|l| !l.persisted).all(|l| l.block_index == 0));
    assert!(events.logs.iter().filter(|l| l.persisted).any(|l| l.block_index > 0));
    assert!(events.logs.iter().all(|l| l.topic_count <= 4 && l.topic0.len() == 32 || l.topic_count == 0));
    // One reverted transaction: every frame attempted, none persisted.
    let failed: Vec<_> = events
        .transactions
        .iter()
        .filter(|t| t.status != pb::TransactionStatus::Succeeded as i32)
        .collect();
    assert_eq!(failed.len(), 1);
    let failed = failed[0];
    assert_eq!(
        (failed.index, failed.status, failed.call_count, failed.reverted_call_count),
        (59, pb::TransactionStatus::Reverted as i32, 11, 11)
    );
    assert!(events
        .calls
        .iter()
        .filter(|c| c.transaction_index == 59 && c.scope == pb::Scope::Transaction as i32)
        .all(|c| !c.persisted && c.state_reverted));
    assert_eq!(failed.receipt_log_count, 0);
    // Reverted children inside succeeded transactions.
    assert_eq!(events.calls.iter().filter(|c| c.state_reverted).count(), 87);
    assert_eq!(events.calls.iter().filter(|c| c.persisted).count(), 766);
    // Transaction types recorded raw and named: legacy, dynamic fee, one blob.
    assert_eq!(events.transactions.iter().filter(|t| t.r#type == pb::TransactionType::Blob as i32).count(), 1);
    let blob = events.transactions.iter().find(|t| t.type_raw == 3).unwrap();
    assert!(blob.blob_count > 0);
    // A nested CREATE persisted code; root creation is a separate fact.
    assert_eq!(events.code_changes.len(), 1);
    let created = &events.code_changes[0];
    assert_eq!(
        (created.kind, created.persisted, created.transaction_index, created.old_code_size),
        (pb::CodeChangeKind::Created as i32, true, 41, 0)
    );
    assert!(created.new_code_size > 0 && created.delegation_target.is_empty());
    assert!(events.transactions.iter().all(|t| t.created_contract.is_empty()));
    assert_eq!(events.calls.iter().filter(|c| c.call_type == pb::CallType::Create as i32).count(), 1);
    // EIP-7702 delegated frames are visible through the code address.
    assert_eq!(events.calls.iter().filter(|c| !c.address_delegates_to.is_empty()).count(), 22);
    // The system call is a separate scope with no transaction hash.
    let system: Vec<_> = events.calls.iter().filter(|c| c.scope == pb::Scope::SystemCall as i32).collect();
    assert_eq!(system.len(), 1);
    assert!(system[0].transaction_hash.is_empty() && system[0].persisted);
    // Selector only by default; sizes are still recorded.
    assert!(events.calls.iter().all(|c| c.input.is_empty() && c.return_data.is_empty()));
    assert!(events.calls.iter().any(|c| c.input_size > 4 && c.input_selector.len() == 4));
    assert!(events.transactions.iter().all(|t| t.input.is_empty()));
    // Deterministic order.
    assert!(events.transactions.windows(2).all(|w| w[0].index < w[1].index));
    assert!(events
        .calls
        .windows(2)
        .all(|w| (w[0].scope, w[0].transaction_index, w[0].index) < (w[1].scope, w[1].transaction_index, w[1].index)));
}

#[test]
fn root_create_records_the_created_contract_and_nested_creates_do_not() {
    let block = eth::Block::decode(DEPLOYMENT).unwrap();
    let events = project(&block, &config()).unwrap();
    let tx = events.transactions.iter().find(|t| !t.created_contract.is_empty()).unwrap();
    // The producer records the created account as `to` for a creation.
    assert_eq!(tx.to, tx.created_contract);
    // The root frame is the first call of the transaction (depth 0).
    let root = events
        .calls
        .iter()
        .filter(|c| c.transaction_index == tx.index && c.scope == pb::Scope::Transaction as i32)
        .min_by_key(|c| c.index)
        .unwrap();
    assert_eq!(root.depth, 0);
    assert_eq!(
        (root.call_type, root.address.clone()),
        (pb::CallType::Create as i32, tx.created_contract.clone())
    );
    assert!(events
        .code_changes
        .iter()
        .any(|c| c.address == tx.created_contract && c.kind == pb::CodeChangeKind::Created as i32 && c.persisted));

    let block = eth::Block::decode(CLONE_CREATION).unwrap();
    let events = project(&block, &config()).unwrap();
    let creates = events.calls.iter().filter(|c| c.call_type == pb::CallType::Create as i32).count();
    assert!(creates >= 1);
    let created_codes = events
        .code_changes
        .iter()
        .filter(|c| c.kind == pb::CodeChangeKind::Created as i32 && c.persisted)
        .count();
    assert_eq!(created_codes, creates);
    assert!(events
        .transactions
        .iter()
        .all(|t| t.created_contract.is_empty() || !t.to.is_empty() || t.created_contract.len() == 20));
}

#[test]
fn failed_setcode_transactions_expose_applied_authorizations_without_persisted_frames() {
    for bytes in FAILED_SETCODE {
        let trace = eth::TransactionTrace::decode(bytes).unwrap();
        let mut block = block();
        block.transaction_traces = vec![trace.clone()];
        let events = project(&block, &config()).unwrap();
        let tx = &events.transactions[0];
        assert_eq!(tx.r#type, pb::TransactionType::SetCode as i32);
        assert_ne!(tx.status, pb::TransactionStatus::Succeeded as i32);
        assert_eq!(tx.set_code_authorization_count, trace.set_code_authorizations.len() as u32);
        assert!(!events.set_code_authorizations.is_empty());
        assert!(events
            .set_code_authorizations
            .iter()
            .all(|a| a.applied != a.discarded && a.authority.len() == 20));
        assert_eq!(
            hex::encode(&events.set_code_authorizations[0].authority),
            "417204ea716dfc4427bf9883521c820b036cdb7a"
        );
        assert!(events.calls.iter().all(|c| !c.persisted));
        assert!(events.calls[0].nonce_change_count > 0);
    }
}

#[test]
fn receipt_disagreement_fails_closed() {
    let mut b = block();
    let mut t = tx(vec![eth::Call {
        logs: vec![log(4, 3, 10)],
        ..Default::default()
    }]);
    t.receipt = Some(eth::TransactionReceipt::default()); // zero receipt logs
    b.transaction_traces = vec![t];
    assert!(project(&b, &config()).unwrap_err().to_string().contains("receipt logs disagree"));
    b.transaction_traces[0].receipt.as_mut().unwrap().logs = vec![log(4, 3, 10)];
    let events = project(&b, &config()).unwrap();
    assert_eq!((events.logs.len(), events.logs[0].topic_count, events.logs[0].data_size), (1, 3, 64));
    assert!(events.logs[0].topic3.is_empty() && events.logs[0].persisted);
}

#[test]
fn captured_version_four_logs_reconcile_with_receipts() {
    let b = eth::Block::decode(include_bytes!("../../../erc20/balances/tests/fixtures/hlbp-active/104727184.pb").as_slice()).unwrap();
    assert_eq!(b.ver, 4);
    let config = parse(r#"{"chain_id":56,"producer_versions":[4]}"#).unwrap();
    let events = project(&b, &config).unwrap();
    assert!(events.logs.iter().any(|log| log.persisted));
    assert_eq!(
        events.logs.iter().filter(|log| log.persisted).count(),
        b.transaction_traces
            .iter()
            .filter_map(|tx| tx.receipt.as_ref())
            .map(|r| r.logs.len())
            .sum::<usize>()
    );
}

#[test]
fn receipt_validation_checks_every_log_field_even_when_log_output_is_disabled() {
    let captured = eth::Block::decode(FULL_BLOCK).unwrap();
    let tx_position = captured
        .transaction_traces
        .iter()
        .position(|tx| tx.receipt.as_ref().is_some_and(|r| !r.logs.is_empty()))
        .unwrap();
    type Mutation = fn(&mut eth::Log);
    let mutations: [(&str, Mutation); 7] = [
        ("address", |l| l.address[0] ^= 1),
        ("topic", |l| l.topics[0][0] ^= 1),
        ("topic count", |l| {
            l.topics.pop();
        }),
        ("data", |l| l.data.push(1)),
        ("transaction log index", |l| l.index += 1),
        ("block log index", |l| l.block_index += 1),
        ("ordinal", |l| l.ordinal += 1),
    ];
    for params in [
        r#"{"chain_id":56,"producer_versions":[5]}"#,
        r#"{"chain_id":56,"producer_versions":[5],"include_calls":false,"include_logs":false}"#,
    ] {
        let config = parse(params).unwrap();
        for (name, mutate) in mutations {
            let mut tampered = captured.clone();
            mutate(&mut tampered.transaction_traces[tx_position].receipt.as_mut().unwrap().logs[0]);
            let error = project(&tampered, &config).unwrap_err().to_string();
            assert!(error.contains("receipt logs disagree"), "{name}: {error}");
            assert!(error.contains(&format!("transaction {} position 0", tampered.transaction_traces[tx_position].index)));
        }
    }
    let mut reordered = captured.clone();
    let receipt = reordered
        .transaction_traces
        .iter_mut()
        .filter_map(|tx| tx.receipt.as_mut())
        .find(|r| r.logs.len() > 1)
        .unwrap();
    receipt.logs.swap(0, 1);
    assert!(project(&reordered, &config()).unwrap_err().to_string().contains("receipt logs disagree"));
}

#[test]
fn receipt_validation_uses_canonical_trace_order_and_excludes_reverted_logs() {
    let mut first = log(1, 1, 10);
    first.index = 0;
    let mut child = log(2, 2, 20);
    child.index = 1;
    child.block_index = 1;
    let mut last = log(1, 1, 30);
    last.index = 3; // The reverted log occupies an attempted index only.
    last.block_index = 2;
    let attempted = log(3, 1, 25);
    let mut t = tx(vec![
        eth::Call {
            index: 1,
            logs: vec![last.clone(), first.clone()],
            ..Default::default()
        },
        eth::Call {
            index: 2,
            logs: vec![child.clone()],
            ..Default::default()
        },
        eth::Call {
            index: 3,
            state_reverted: true,
            logs: vec![attempted.clone()],
            ..Default::default()
        },
    ]);
    t.receipt.as_mut().unwrap().logs = vec![first, child, last];
    let mut b = block();
    b.transaction_traces = vec![t];
    let expected = project(&b, &config()).unwrap();
    assert_eq!(expected.logs.iter().filter(|l| l.persisted).count(), 3);
    b.transaction_traces[0].calls.reverse();
    assert_eq!(project(&b, &config()).unwrap(), expected);
    b.transaction_traces[0].receipt.as_mut().unwrap().logs.insert(2, attempted);
    assert!(project(&b, &config()).unwrap_err().to_string().contains("receipt logs disagree"));

    for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
        b.transaction_traces[0].status = status as i32;
        b.transaction_traces[0].receipt.as_mut().unwrap().logs.clear();
        assert!(project(&b, &config()).unwrap().logs.iter().all(|log| !log.persisted));
    }
}

#[test]
fn ambiguous_persisted_log_ordinals_are_rejected() {
    for ordinal in [0, 10] {
        let logs = vec![log(1, 1, 10), log(2, 1, ordinal)];
        let mut b = block();
        let mut t = tx(vec![eth::Call {
            logs: logs.clone(),
            ..Default::default()
        }]);
        t.receipt.as_mut().unwrap().logs = logs;
        b.transaction_traces = vec![t];
        assert!(project(&b, &config()).unwrap_err().to_string().contains("persisted log ordinals"));
    }
}

#[test]
fn only_reviewed_producer_versions_can_be_configured() {
    for versions in ["[]", "[0]", "[-1]", "[3]", "[6]", "[999]", "[4,999]"] {
        assert!(parse(&format!(r#"{{"chain_id":56,"producer_versions":{versions}}}"#)).is_err());
    }
    for versions in ["[4]", "[5]", "[4,5]"] {
        assert!(parse(&format!(r#"{{"chain_id":56,"producer_versions":{versions}}}"#)).is_ok());
    }
}

#[test]
fn reverted_frames_system_calls_and_block_records_carry_their_scope_and_persistence() {
    let mut b = block();
    let mut t = tx(vec![
        eth::Call {
            index: 0,
            logs: vec![log(4, 1, 10)],
            ..Default::default()
        },
        eth::Call {
            index: 1,
            parent_index: 0,
            depth: 1,
            state_reverted: true,
            status_reverted: true,
            status_failed: true,
            failure_reason: "execution reverted".into(),
            logs: vec![log(5, 2, 0)],
            code_changes: vec![eth::CodeChange {
                address: vec![6; 20],
                new_hash: vec![9; 32],
                new_code: vec![0x60, 0x00],
                ordinal: 12,
                ..Default::default()
            }],
            ..Default::default()
        },
    ]);
    t.receipt.as_mut().unwrap().logs = vec![log(4, 1, 10)];
    b.transaction_traces = vec![t];
    b.system_calls = vec![
        eth::Call {
            index: 0,
            logs: vec![log(7, 1, 30)],
            ..Default::default()
        },
        eth::Call {
            index: 1,
            state_reverted: true,
            ..Default::default()
        },
    ];
    b.code_changes = vec![eth::CodeChange {
        address: vec![0x10; 20],
        old_hash: vec![1; 32],
        old_code: vec![1],
        new_hash: vec![2; 32],
        new_code: vec![2, 2],
        ordinal: 40,
    }];
    let events = project(&b, &config()).unwrap();
    let reverted = events.calls.iter().find(|c| c.index == 1 && c.scope == pb::Scope::Transaction as i32).unwrap();
    assert!(reverted.state_reverted && !reverted.persisted && reverted.failure_reason == "execution reverted");
    assert_eq!(events.logs.iter().filter(|l| !l.persisted).count(), 1);
    let attempted_code = events.code_changes.iter().find(|c| c.scope == pb::Scope::Transaction as i32).unwrap();
    assert!(!attempted_code.persisted && attempted_code.kind == pb::CodeChangeKind::Created as i32);
    let system: Vec<_> = events.calls.iter().filter(|c| c.scope == pb::Scope::SystemCall as i32).collect();
    assert_eq!(system.len(), 2);
    assert!(system[0].persisted && !system[1].persisted);
    assert!(events
        .logs
        .iter()
        .any(|l| l.scope == pb::Scope::SystemCall as i32 && l.transaction_hash.is_empty() && l.persisted));
    let block_code = events.code_changes.iter().find(|c| c.scope == pb::Scope::Block as i32).unwrap();
    assert!(block_code.persisted && block_code.kind == pb::CodeChangeKind::Replaced as i32 && block_code.transaction_hash.is_empty());
    assert_eq!((events.clocks[0].system_call_count, events.clocks[0].code_change_count), (2, 2));
}

#[test]
fn code_change_kinds_follow_eip_7702_and_eip_6780_shapes() {
    let mut target = vec![0xef, 0x01, 0x00];
    target.extend([0xab; 20]);
    let set = eth::CodeChange {
        new_code: target.clone(),
        new_hash: vec![1; 32],
        ..Default::default()
    };
    assert_eq!(code_change_kind(&set), (pb::CodeChangeKind::DelegationSet, vec![0xab; 20]));
    let cleared = eth::CodeChange {
        old_code: target.clone(),
        old_hash: vec![1; 32],
        new_hash: keccak(&[]).to_vec(),
        ..Default::default()
    };
    assert_eq!(code_change_kind(&cleared).0, pb::CodeChangeKind::DelegationCleared);
    let destroyed = eth::CodeChange {
        old_code: vec![0x60],
        old_hash: vec![1; 32],
        ..Default::default()
    };
    assert_eq!(code_change_kind(&destroyed).0, pb::CodeChangeKind::Cleared);
    let replaced = eth::CodeChange {
        old_code: vec![0x60],
        old_hash: vec![1; 32],
        new_code: vec![0x61],
        new_hash: vec![2; 32],
        ..Default::default()
    };
    assert_eq!(code_change_kind(&replaced).0, pb::CodeChangeKind::Replaced);
    let created = eth::CodeChange {
        old_hash: keccak(&[]).to_vec(),
        new_code: vec![0x61],
        new_hash: vec![2; 32],
        ..Default::default()
    };
    assert_eq!(code_change_kind(&created).0, pb::CodeChangeKind::Created);
}

#[test]
fn parameters_select_payloads_and_fail_closed() {
    assert!(parse(r#"{"chain_id":56,"producer_versions":[]}"#).is_err());
    assert!(parse(r#"{"chain_id":0,"producer_versions":[5]}"#).is_err());
    assert!(parse(r#"{"chain_id":56,"producer_versions":[5],"extra":1}"#).is_err());
    let full = parse(r#"{"chain_id":56,"producer_versions":[5],"include_input":true,"include_return_data":true}"#).unwrap();
    let mut b = block();
    b.transaction_traces = vec![tx(vec![eth::Call {
        input: vec![0xa9, 0x05, 0x9c, 0xbb, 1, 2, 3],
        return_data: vec![1],
        ..Default::default()
    }])];
    b.transaction_traces[0].input = vec![0xa9, 0x05, 0x9c, 0xbb, 1, 2, 3];
    let events = project(&b, &full).unwrap();
    assert_eq!(
        (
            events.calls[0].input.len(),
            events.calls[0].return_data.len(),
            events.transactions[0].input.len()
        ),
        (7, 1, 7)
    );
    assert_eq!(hex::encode(&events.calls[0].input_selector), "a9059cbb");
    let lean = project(&b, &config()).unwrap();
    assert!(lean.calls[0].input.is_empty() && lean.calls[0].input_size == 7);
    let none = parse(r#"{"chain_id":56,"producer_versions":[5],"include_calls":false,"include_logs":false}"#).unwrap();
    let events = project(&b, &none).unwrap();
    assert!(events.calls.is_empty() && events.logs.is_empty() && events.transactions.len() == 1);
    // Unlisted producer versions and non-Extended blocks are refused.
    b.ver = 4;
    assert!(project(&b, &config()).unwrap_err().to_string().contains("producer version"));
    b.ver = 5;
    b.detail_level = eth::block::DetailLevel::DetaillevelBase as i32;
    assert!(project(&b, &config()).is_err());
    b = block();
    b.transaction_traces = vec![eth::TransactionTrace {
        status: 1,
        hash: vec![1; 31],
        calls: vec![eth::Call::default()],
        ..Default::default()
    }];
    assert!(project(&b, &config()).is_err());
}

#[test]
fn empty_block_still_emits_exactly_one_clock() {
    let events = project(&block(), &config()).unwrap();
    assert_eq!(events.clocks.len(), 1);
    assert!(events.transactions.is_empty() && events.calls.is_empty());
    assert_eq!(events.clocks[0].parameters_sha256, config().parameters_sha256);
}
