use super::*;
use prost::Message;

const POOL_PARAMS: &str = include_str!("../tests/fixtures/bsc-aave-v3-pool.json");
const BORROW: &[u8] = include_bytes!("../tests/fixtures/122288242-tx32-borrow-usdt.pb");
const SUPPLY_BTCB: &[u8] = include_bytes!("../tests/fixtures/122288773-tx51-supply-btcb.pb");
const WITHDRAW: &[u8] = include_bytes!("../../balance-state/tests/fixtures/122288220-tx71-ausdt-withdraw.pb");
const MULTI: &[u8] = include_bytes!("../../balance-state/tests/fixtures/122288932-tx1-ausdt-ausdc-two-reserves.pb");
const POOL: &str = "6807dc923806fe8fd134338eabca509979a7e0cb";
const USDT: &str = "55d398326f99059ff775485246999027b3197955";
const USDC: &str = "8ac76a51cc950d9822d68b83fe1ad97b32cd580d";

fn config() -> Config {
    parse(POOL_PARAMS).unwrap()
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
fn decode_block(bytes: &[u8]) -> eth::Block {
    let block = eth::Block::decode(bytes).unwrap();
    assert_eq!((block.ver, block.transaction_traces.len()), (5, 1));
    block
}
fn block() -> eth::Block {
    eth::Block {
        ver: 5,
        number: 100,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number: 100,
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
        ..Default::default()
    }
}
fn padded(byte: u8) -> Vec<u8> {
    let mut t = vec![0u8; 32];
    t[12..].fill(byte);
    t
}
fn word(v: u64) -> Vec<u8> {
    let mut w = vec![0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}
fn pool_log(topic0: &str, topics: Vec<Vec<u8>>, data: Vec<Vec<u8>>, ordinal: u64) -> eth::Log {
    let mut all = vec![hex::decode(topic0).unwrap()];
    all.extend(topics);
    eth::Log {
        address: hex::decode(POOL).unwrap(),
        topics: all,
        data: data.concat(),
        ordinal,
        ..Default::default()
    }
}

#[test]
fn captured_borrow_supply_and_withdraw_keep_actor_beneficiary_and_recipient_apart() {
    let events = project(&decode_block(BORROW), &config()).unwrap();
    assert_eq!(events.actions.len(), 1);
    let a = &events.actions[0];
    assert_eq!((a.kind, a.persisted, a.ordinal, a.call_index), (pb::ActionKind::Borrow as i32, true, 2549, 4));
    assert_eq!(hex::encode(&a.reserve), USDT);
    assert_eq!(hex::encode(&a.actor), "13f4e5bd5c0da823190f58db3ee98dfa2fa09042");
    assert_eq!(a.actor, a.beneficiary);
    assert!(a.recipient.is_empty());
    assert_eq!(
        (&*a.amount, a.interest_rate_mode, &*a.borrow_rate, a.referral_code),
        ("50000000000000000000", 2, "39024191965121689668949461", 0)
    );
    assert_eq!((a.epoch, hex::encode(&a.pool)), (1, POOL.into()));

    let events = project(&decode_block(SUPPLY_BTCB), &config()).unwrap();
    let a = &events.actions[0];
    assert_eq!((a.kind, a.persisted), (pb::ActionKind::Supply as i32, true));
    assert_eq!(hex::encode(&a.reserve), "7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c");
    assert_eq!((&*a.amount, a.interest_rate_mode), ("9431971302831856", 0));
    assert!(a.borrow_rate.is_empty() && a.recipient.is_empty());

    let events = project(&decode_block(WITHDRAW), &config()).unwrap();
    let a = &events.actions[0];
    assert_eq!((a.kind, a.persisted, a.ordinal), (pb::ActionKind::Withdraw as i32, true, 3362));
    assert_eq!(hex::encode(&a.recipient), "e88a4b3c49d4bf48d938cee1400e6806b2550386");
    assert_eq!((a.actor.clone(), a.beneficiary.clone()), (a.recipient.clone(), a.recipient.clone()));
    assert_eq!(a.amount, "5527010534175222644");
    assert_eq!(events.clocks[0].action_count, 1);
}

#[test]
fn reverted_attempts_and_persisted_repeats_are_both_facts_in_one_transaction() {
    let block = decode_block(MULTI);
    let events = project(&block, &config()).unwrap();
    assert_eq!(events.actions.len(), 7);
    let kinds: Vec<(i32, bool)> = events.actions.iter().map(|a| (a.kind, a.persisted)).collect();
    assert_eq!(
        kinds,
        vec![
            (pb::ActionKind::Supply as i32, false),
            (pb::ActionKind::Withdraw as i32, false),
            (pb::ActionKind::Borrow as i32, false),
            (pb::ActionKind::Supply as i32, true),
            (pb::ActionKind::Withdraw as i32, true),
            (pb::ActionKind::Borrow as i32, true),
            (pb::ActionKind::Supply as i32, false),
        ]
    );
    // The attempted and persisted supply carry the same facts; only
    // `persisted` and identity differ.
    assert_eq!(
        (events.actions[0].amount.clone(), events.actions[0].reserve.clone()),
        (events.actions[3].amount.clone(), events.actions[3].reserve.clone())
    );
    assert_eq!(hex::encode(&events.actions[3].reserve), USDT);
    assert_eq!(hex::encode(&events.actions[5].reserve), USDC);
    assert!(events.actions.windows(2).all(|w| w[0].ordinal < w[1].ordinal));
    assert!(events
        .actions
        .iter()
        .all(|a| hex::encode(&a.actor) == "e5ec006540be4f7cbb2cbc7be79708a6d96f90dc"));
    // Persisted rows only when attempts are excluded.
    let lean = parse(&POOL_PARAMS.replace(r#""producer_versions": [5],"#, r#""producer_versions": [5], "include_attempted": false,"#)).unwrap();
    let events = project(&block, &lean).unwrap();
    assert_eq!(events.actions.len(), 3);
    assert!(events.actions.iter().all(|a| a.persisted));
}

#[test]
fn repay_liquidation_and_flash_loan_decode_from_the_pinned_shapes() {
    let mut b = block();
    let repay = pool_log(REPAY_TOPIC, vec![padded(0x11), padded(0x22), padded(0x33)], vec![word(500), word(1)], 10);
    let liquidation = pool_log(
        LIQUIDATION_CALL_TOPIC,
        vec![padded(0x44), padded(0x11), padded(0x22)],
        vec![word(700), word(90), padded(0x55), word(0)],
        11,
    );
    let mut referral = vec![0u8; 32];
    referral[31] = 7;
    let flash = pool_log(
        FLASH_LOAN_TOPIC,
        vec![padded(0x66), padded(0x11), referral],
        vec![padded(0x77), word(1000), word(0), word(9)],
        12,
    );
    b.transaction_traces = vec![tx(vec![eth::Call {
        logs: vec![repay, liquidation, flash],
        ..Default::default()
    }])];
    let events = project(&b, &config()).unwrap();
    assert_eq!(events.actions.len(), 3);
    let r = &events.actions[0];
    assert_eq!(
        (r.kind, r.reserve.clone(), r.beneficiary.clone(), r.actor.clone(), &*r.amount, r.use_atokens),
        (pb::ActionKind::Repay as i32, vec![0x11; 20], vec![0x22; 20], vec![0x33; 20], "500", true)
    );
    let l = &events.actions[1];
    assert_eq!(
        (l.kind, l.collateral_asset.clone(), l.reserve.clone(), l.beneficiary.clone(), l.actor.clone()),
        (
            pb::ActionKind::LiquidationCall as i32,
            vec![0x44; 20],
            vec![0x11; 20],
            vec![0x22; 20],
            vec![0x55; 20]
        )
    );
    assert_eq!((&*l.amount, &*l.liquidated_collateral_amount, l.receive_atoken), ("700", "90", false));
    let f = &events.actions[2];
    assert_eq!(
        (f.kind, f.beneficiary.clone(), f.reserve.clone(), f.actor.clone()),
        (pb::ActionKind::FlashLoan as i32, vec![0x66; 20], vec![0x11; 20], vec![0x77; 20])
    );
    assert_eq!((&*f.amount, f.interest_rate_mode, &*f.premium, f.referral_code), ("1000", 0, "9", 7));
}

#[test]
fn unbound_shapes_pointer_writes_and_code_changes_fail_closed() {
    let cfg = config();
    let pool = &cfg.pools[0];
    let mut b = block();
    // Supply with three data words is not the bound IPool shape.
    let wrong = pool_log(
        SUPPLY_TOPIC,
        vec![padded(0x11), padded(0x22), word(0)],
        vec![padded(0x33), word(1), word(2)],
        10,
    );
    b.transaction_traces = vec![tx(vec![eth::Call {
        logs: vec![wrong],
        ..Default::default()
    }])];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("shape differs"));
    // A bool field outside {0,1} and a non-padded address topic.
    let bad_bool = pool_log(REPAY_TOPIC, vec![padded(0x11), padded(0x22), padded(0x33)], vec![word(500), word(2)], 10);
    b.transaction_traces = vec![tx(vec![eth::Call {
        logs: vec![bad_bool],
        ..Default::default()
    }])];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("bool"));
    let mut unpadded = pool_log(WITHDRAW_TOPIC, vec![padded(0x11), padded(0x22), padded(0x33)], vec![word(1)], 10);
    unpadded.topics[1][0] = 1;
    b.transaction_traces = vec![tx(vec![eth::Call {
        logs: vec![unpadded],
        ..Default::default()
    }])];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("padded address"));
    // Other Pool events and other emitters are ignored.
    let other = eth::Log {
        address: hex::decode(POOL).unwrap(),
        topics: vec![vec![0xab; 32]],
        ..Default::default()
    };
    let foreign = pool_log(SUPPLY_TOPIC, vec![padded(0x11), padded(0x22), word(0)], vec![padded(0x33), word(1)], 11);
    let mut foreign = foreign;
    foreign.address = vec![0x99; 20];
    b.transaction_traces = vec![tx(vec![eth::Call {
        logs: vec![other, foreign],
        ..Default::default()
    }])];
    assert!(project(&b, &cfg).unwrap().actions.is_empty());
    // Implementation pointer write and code changes.
    b.transaction_traces = vec![tx(vec![eth::Call {
        storage_changes: vec![eth::StorageChange {
            address: pool.address.clone(),
            key: pool.implementation_slot.to_vec(),
            old_value: vec![1],
            new_value: vec![2],
            ordinal: 5,
        }],
        ..Default::default()
    }])];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("pointer"));
    for address in [&pool.address, &pool.implementation] {
        b.transaction_traces = vec![tx(vec![eth::Call {
            code_changes: vec![eth::CodeChange {
                address: address.clone(),
                old_hash: vec![1; 32],
                new_hash: vec![2; 32],
                ordinal: 5,
                ..Default::default()
            }],
            ..Default::default()
        }])];
        assert!(project(&b, &cfg).unwrap_err().to_string().contains("code changed"));
    }
    // A reverted pointer write never persisted and is not a guard failure.
    b.transaction_traces = vec![tx(vec![eth::Call {
        state_reverted: true,
        storage_changes: vec![eth::StorageChange {
            address: pool.address.clone(),
            key: pool.implementation_slot.to_vec(),
            old_value: vec![1],
            new_value: vec![2],
            ordinal: 5,
        }],
        ..Default::default()
    }])];
    assert!(project(&b, &cfg).is_ok());
}

#[test]
fn parameters_are_explicit_and_an_unbound_pool_emits_only_the_clock() {
    assert!(parse(r#"{"chain_id":56,"producer_versions":[5],"pools":[]}"#).unwrap().pools.is_empty());
    assert!(parse(r#"{"chain_id":56,"producer_versions":[]}"#).is_err());
    assert!(parse(r#"{"chain_id":56,"producer_versions":[5],"x":1}"#).is_err());
    let mut v: serde_json::Value = serde_json::from_str(POOL_PARAMS).unwrap();
    v["pools"][0]["epoch"] = 0.into();
    assert!(parse(&v.to_string()).is_err());
    let mut v: serde_json::Value = serde_json::from_str(POOL_PARAMS).unwrap();
    let duplicate = v["pools"][0].clone();
    v["pools"].as_array_mut().unwrap().push(duplicate);
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("duplicate"));
    let none = parse(r#"{"chain_id":56,"producer_versions":[5]}"#).unwrap();
    let events = project(&decode_block(BORROW), &none).unwrap();
    assert!(events.actions.is_empty() && events.clocks.len() == 1);
    let mut b = block();
    b.ver = 4;
    assert!(project(&b, &config()).unwrap_err().to_string().contains("producer version"));
}

#[test]
fn every_fixture_carries_its_recorded_identity() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("../tests/fixtures/cases.json")).unwrap();
    let listed = cases["cases"].as_array().unwrap();
    assert_eq!(listed.len(), 2);
    for (file, bytes) in [("122288242-tx32-borrow-usdt.pb", BORROW), ("122288773-tx51-supply-btcb.pb", SUPPLY_BTCB)] {
        let case = listed.iter().find(|c| c["file"] == file).unwrap();
        let block = decode_block(bytes);
        assert_eq!(case["block"], block.number);
        assert_eq!(case["block_hash"], format!("0x{}", hex::encode(&block.hash)));
        assert_eq!(case["transaction"], format!("0x{}", hex::encode(&block.transaction_traces[0].hash)));
        assert_eq!(case["fixture_sha256"], hex::encode(sha2::Sha256::digest(bytes)));
    }
}

#[test]
fn topic_constants_match_the_pinned_signatures() {
    for (sig, expected) in [
        ("Supply(address,address,address,uint256,uint16)", SUPPLY_TOPIC),
        ("Withdraw(address,address,address,uint256)", WITHDRAW_TOPIC),
        ("Borrow(address,address,address,uint256,uint8,uint256,uint16)", BORROW_TOPIC),
        ("Repay(address,address,address,uint256,bool)", REPAY_TOPIC),
        ("LiquidationCall(address,address,address,uint256,uint256,address,bool)", LIQUIDATION_CALL_TOPIC),
        ("FlashLoan(address,address,address,uint256,uint8,uint256,uint16)", FLASH_LOAN_TOPIC),
    ] {
        assert_eq!(hex::encode(keccak(sig.as_bytes())), expected, "{sig}");
    }
}
