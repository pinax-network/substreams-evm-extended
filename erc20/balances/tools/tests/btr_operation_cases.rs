#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::btr_proof::{self as proof, cases::Proof, CAPTURES};
use erc20_balances_tools::ptoken_proof::cases::{call, mapping, role, w};
use primitive_types::U256;
use serde_json::Value;
use std::sync::OnceLock;
const CAPTURE: &[u8] = include_bytes!("../../tests/fixtures/btr-operation-proof/implementation-capture.json");
const COMPILED: &[u8] = include_bytes!("../../tests/fixtures/btr-operation-proof/implementation-compiler-output.json");
fn records() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let capture = proof::verify_capture(CAPTURE, &CAPTURES[0]).unwrap();
        proof::verify_compiled(&capture, COMPILED, &CAPTURES[0]).unwrap();
        let runtime = proof::runtime(&capture).unwrap();
        let mut p = Proof::new(&runtime);
        let mut attempted = 0;
        let mut save = |n: usize, record: &Value| {
            assert_eq!(n, attempted + 1);
            assert!(record["execution"]["trace"].as_array().is_some());
            attempted = n;
            Ok(())
        };
        p.constructor(&proof::bytes(&capture["creationBytecode"]["onchainBytecode"]).unwrap(), &mut save)
            .unwrap();
        p.matrix(&mut save).unwrap();
        assert_eq!(p.calls, attempted);
        assert_eq!(p.calls, p.cases.len());
        p.cases
    })
}
fn record(name: &str, sig: &str) -> &'static Value {
    records()
        .iter()
        .find(|r| r["name"] == name && r["signature"] == sig)
        .unwrap_or_else(|| panic!("missing {name}: {sig}"))
}
fn number(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().strip_prefix("0x").unwrap(), 16).unwrap()
}
fn state(v: &Value) -> proof::vm::State {
    v.as_array().unwrap().iter().map(|s| (number(&s["key"]), number(&s["value"]))).collect()
}
#[test]
fn btr_actual_local_operation_matrix() {
    assert!(records().len() > 500, "complete constructor, operation, getter and rollback matrix");
    for r in records() {
        let implementation = r["name"] == "implementation_constructor" || r["name"] == "locked_implementation_initializer";
        assert_eq!(
            r["address"],
            w(proof::address(if implementation { proof::IMPLEMENTATION } else { proof::PROXY }))
        );
        assert_eq!(
            r["self_code_size"],
            if r["name"] == "implementation_constructor" {
                0
            } else if implementation {
                15308
            } else {
                183
            }
        );
        assert!(matches!(r["execution"]["exit"]["kind"].as_str(), Some("return" | "revert")));
        if r["execution"]["exit"]["kind"] == "revert" {
            assert_eq!(r["execution"]["committed_storage"], r["prestate"], "{}", r["name"]);
            assert_eq!(r["execution"]["committed_logs"], 0);
        }
    }
}
#[test]
fn btr_source_specific_wrapper_noop_events_and_physical_zero_stores() {
    for (name, sig, event, enabled) in [
        ("wrapper_minter_duplicate", "setMinter(address,bool)", "MinterSet(address,bool)", true),
        ("wrapper_minter_absent", "setMinter(address,bool)", "MinterSet(address,bool)", false),
        ("wrapper_burner_duplicate", "setBurner(address,bool)", "BurnerSet(address,bool)", true),
        ("wrapper_burner_absent", "setBurner(address,bool)", "BurnerSet(address,bool)", false),
        ("whitelist_duplicate", "setWhitelister(address,bool)", "WhitelistedSet(address,bool)", true),
        (
            "whitelist_absent_nonempty",
            "setWhitelister(address,bool)",
            "WhitelistedSet(address,bool)",
            false,
        ),
    ] {
        let r = record(name, sig);
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
        let logs = r["execution"]["logs"].as_array().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0]["topics"], serde_json::json!([w(role(event))]));
        let data = hex::decode(logs[0]["data"].as_str().unwrap()).unwrap();
        assert_eq!(U256::from_big_endian(&data[32..]), U256::from(u8::from(enabled)));
    }
    for (name, sig) in [
        ("role_add_zero_empty", "grantRole(bytes32,address)"),
        ("role_remove_zero_sole", "revokeRole(bytes32,address)"),
        ("whitelist_add_zero_empty", "setWhitelister(address,bool)"),
        ("whitelist_remove_zero_sole", "setWhitelister(address,bool)"),
    ] {
        let writes = record(name, sig)["execution"]["writes"].as_array().unwrap();
        assert!(
            writes.iter().any(|s| number(&s["old"]).is_zero() && number(&s["new"]).is_zero()),
            "{name}: physical zero equality"
        );
    }
    for (name, sig) in [
        ("role_duplicate", "grantRole(bytes32,address)"),
        ("role_absent_nonempty", "revokeRole(bytes32,address)"),
    ] {
        let r = record(name, sig);
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
        assert!(r["execution"]["logs"].as_array().unwrap().is_empty());
    }
}
#[test]
fn btr_all_four_incoherent_results_and_attempted_revert_prefix() {
    let bool_key = w(proof::cases::member_key(role("INCOHERENT_ROLE"), 3.into()));
    for (name, sig, bool_change, set_change) in [
        ("incoherent_set_only_grant", "grantRole(bytes32,address)", false, true),
        ("incoherent_bool_only_grant", "grantRole(bytes32,address)", true, false),
        ("incoherent_bool_only_revoke", "revokeRole(bytes32,address)", true, false),
        ("incoherent_set_only_revoke", "revokeRole(bytes32,address)", false, true),
    ] {
        let r = record(name, sig);
        let writes = r["execution"]["writes"].as_array().unwrap();
        assert_eq!(r["execution"]["exit"]["kind"], "return");
        assert_eq!(writes.iter().any(|s| s["key"] == bool_key), bool_change);
        assert_eq!(writes.iter().any(|s| s["key"] != bool_key), set_change);
        assert_eq!(r["execution"]["logs"].as_array().unwrap().len(), usize::from(bool_change));
    }
    let r = record("empty_set_prefix_revert", "revokeRole(bytes32,address)");
    assert_eq!(r["execution"]["writes"].as_array().unwrap().len(), 1);
    assert_eq!(r["execution"]["logs"].as_array().unwrap().len(), 1);
    assert_eq!(r["execution"]["committed_logs"], 0);
    assert_eq!(r["execution"]["exit"]["data"], hex::encode(call("Panic(uint256)", &[0x11.into()])));
}
#[test]
fn btr_distinct_owner_pauser_and_initializer_failure_boundaries() {
    for name in ["owner_cannot_whitelist", "initialized_owner_cannot_whitelist", "owner_cannot_grant_pauser"] {
        let r = records().iter().find(|r| r["name"] == name).unwrap();
        assert_eq!(r["caller"], w(1.into()));
        assert_eq!(r["execution"]["exit"]["kind"], "revert");
    }
    let r = record("initialized_pauser_can_whitelist", "setWhitelister(address,bool)");
    assert_eq!(r["caller"], w(2.into()));
    assert_eq!(r["execution"]["exit"]["kind"], "return");
    let init = record("initialize_distinct_owner_pauser", "initialize(string,string,address,address,uint256)");
    let s = state(&init["execution"]["committed_storage"]);
    assert_eq!(s[&proof::cases::member_key(0.into(), 1.into())], 1.into());
    assert_eq!(s[&proof::cases::member_key(role("PAUSER_ROLE"), 2.into())], 1.into());
    assert_eq!(s[&proof::cases::role_admin(role("PAUSER_ROLE"))], role("PAUSER_ROLE"));
    let late = record("initialize_quota_overflow", "initialize(string,string,address,address,uint256)");
    assert_eq!(late["execution"]["logs"].as_array().unwrap().len(), 4);
    assert_eq!(late["execution"]["committed_storage"], serde_json::json!([]));
    assert_eq!(late["execution"]["committed_logs"], 0);
    let zero = record("initialize_zero_quota", "initialize(string,string,address,address,uint256)");
    assert_eq!(zero["execution"]["exit"]["kind"], "return");
    assert_eq!(
        state(&zero["execution"]["committed_storage"]).get(&554.into()).copied().unwrap_or_default(),
        0.into()
    );
}
#[test]
fn btr_getter_balance_storage_and_whitelist_full_array_abi() {
    for r in records().iter().filter(|r| r["signature"] == "balanceOf(address)") {
        let data = hex::decode(r["calldata"].as_str().unwrap()).unwrap();
        let key = mapping(U256::from_big_endian(&data[4..]), 201.into());
        let s = state(&r["prestate"]);
        assert_eq!(
            r["execution"]["exit"]["data"],
            hex::encode(proof::vm::word(s.get(&key).copied().unwrap_or_default()))
        );
        assert!(r["execution"]["reads"].as_array().unwrap().iter().any(|v| v["key"] == w(key)));
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
    }
    let r = record("whitelist_remove_first", "queryWhitelisted()");
    let data = hex::decode(r["execution"]["exit"]["data"].as_str().unwrap()).unwrap();
    assert_eq!(
        data.chunks_exact(32).map(U256::from_big_endian).collect::<Vec<_>>(),
        vec![32.into(), 2.into(), 5.into(), 4.into()]
    );
}
#[test]
fn btr_corrupted_store_fails_after_saving_attempt() {
    let r = record("role_add_empty", "grantRole(bytes32,address)");
    let pc = r["execution"]["writes"][0]["pc"].as_u64().unwrap() as usize;
    let c = proof::verify_capture(CAPTURE, &CAPTURES[0]).unwrap();
    let mut runtime = proof::runtime(&c).unwrap();
    assert_eq!(runtime[pc], 0x55);
    runtime[pc] = 0x50;
    let mut p = Proof::new(&runtime);
    let mut saved = vec![];
    assert!(p
        .matrix(&mut |_, v| {
            saved.push(v.clone());
            Ok(())
        })
        .is_err());
    assert!(!saved.is_empty());
    assert_eq!(saved[0]["name"], "role_add_empty");
    assert!(saved[0]["execution"]["trace"].as_array().is_some());
}
#[test]
fn btr_missing_self_context_is_harness_failure_not_solidity_revert() {
    let c = proof::verify_capture(CAPTURE, &CAPTURES[0]).unwrap();
    let runtime = proof::runtime(&c).unwrap();
    let pre = proof::vm::State::from([(0.into(), 1.into())]);
    let e = proof::vm::execute(&runtime, &proof::initializer_arguments(), 9.into(), proof::address(proof::PROXY), &pre);
    assert!(matches!(e.exit, proof::vm::Exit::HarnessFailure(_)));
    assert_eq!(e.committed, pre);
    assert!(e.committed_logs.is_empty());
    let e = proof::vm::execute_with_self_code_size(
        &runtime,
        &proof::initializer_arguments(),
        9.into(),
        proof::address(proof::PROXY),
        &pre,
        Some(183),
    );
    assert!(matches!(e.exit, proof::vm::Exit::Revert(_)));
    assert_eq!(e.committed, pre);
}
