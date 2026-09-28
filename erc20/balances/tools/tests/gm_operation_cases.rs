#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::gm_proof::{self as proof, cases::Proof, CAPTURES};
use erc20_balances_tools::ptoken_proof::cases::{call, mapping, role, w};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{path::Path, sync::OnceLock};
const CAPTURE: &[u8] = include_bytes!("../../tests/fixtures/gm-operation-proof/implementation-capture.json");
fn records() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let c = proof::verify_capture(CAPTURE, &CAPTURES[0]).unwrap();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/gm-operation-proof/implementation-compiler-output.json");
        proof::verify_compiled(&c, &std::fs::read(path).unwrap(), &CAPTURES[0]).unwrap();
        let compiler = proof::bytes(&c["runtimeBytecode"]["recompiledBytecode"]).unwrap();
        let captured = proof::runtime(&c).unwrap();
        assert_ne!(compiler, captured, "known CBOR substitution remains explicit");
        assert_eq!(&compiler[..8218], &captured[..8218]);
        let mut p = Proof::new(&compiler);
        let mut saved = 0;
        let mut save = |n: usize, v: &Value| {
            assert_eq!(n, saved + 1);
            assert!(v["execution"]["trace"].is_array());
            if v["name"] != "implementation_constructor" {
                proof::verify_runtime_trace(&v["execution"])?;
            }
            saved = n;
            Ok(())
        };
        p.constructor(&proof::bytes(&c["creationBytecode"]["recompiledBytecode"]).unwrap(), &mut save)
            .unwrap();
        assert_eq!(p.calls, 1);
        p.matrix(&mut save).unwrap();
        assert_eq!(p.calls, saved);
        assert_eq!(p.calls, p.cases.len());
        let mut q = Proof::new(&captured);
        q.matrix(&mut |_, v| proof::verify_runtime_trace(&v["execution"])).unwrap();
        assert_eq!(
            p.cases[1..],
            q.cases,
            "every actual trace, state, effect and exit is equal across the declared metadata substitution"
        );
        p.cases
    })
}
fn record(name: &str, signature: &str) -> &'static Value {
    records()
        .iter()
        .find(|r| r["name"] == name && r["signature"] == signature)
        .unwrap_or_else(|| panic!("missing {name}: {signature}"))
}
fn number(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().strip_prefix("0x").unwrap(), 16).unwrap()
}
fn state(v: &Value) -> proof::vm::State {
    v.as_array().unwrap().iter().map(|s| (number(&s["key"]), number(&s["value"]))).collect()
}
#[test]
fn gm_complete_local_matrix_and_metadata_equivalence() {
    assert!(
        records().len() > 500,
        "constructor, role operations, initialization, raw getters and rollback controls"
    );
    for r in records() {
        let kind = r["execution"]["exit"]["kind"].as_str().unwrap();
        assert!(matches!(kind, "return" | "revert" | "harness_failure"));
        assert_eq!(kind == "harness_failure", r["scope"] == "unsupported_path_control");
        if kind != "return" {
            assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
            assert_eq!(r["execution"]["committed_logs"], 0);
        }
    }
    for account in [proof::PROXY, proof::PROXY_B, proof::IMPLEMENTATION] {
        assert!(records()
            .iter()
            .any(|r| r["address"] == w(proof::address(account)) && r["self_code_size"] == if account == proof::IMPLEMENTATION { 8271 } else { 824 }));
    }
    let ctor = record("implementation_constructor", "constructor()");
    assert_eq!(ctor["self_code_size"], 0);
    assert_eq!(state(&ctor["execution"]["committed_storage"]), proof::vm::State::from([(0.into(), 255.into())]));
}
#[test]
fn gm_zero_physical_stores_and_true_noops() {
    for (name, sig) in [
        ("role_add_zero_empty", "grantRole(bytes32,address)"),
        ("role_remove_zero_sole", "revokeRole(bytes32,address)"),
        ("role_remove_zero_tail_swap", "revokeRole(bytes32,address)"),
    ] {
        assert!(
            record(name, sig)["execution"]["writes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| number(&s["old"]).is_zero() && number(&s["new"]).is_zero()),
            "{name}"
        );
    }
    for (name, sig) in [
        ("role_duplicate", "grantRole(bytes32,address)"),
        ("role_duplicate_zero", "grantRole(bytes32,address)"),
        ("role_absent_empty", "revokeRole(bytes32,address)"),
        ("renounce_absent", "renounceRole(bytes32,address)"),
    ] {
        let r = record(name, sig);
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
        assert!(r["execution"]["logs"].as_array().unwrap().is_empty());
        let data = hex::decode(r["calldata"].as_str().unwrap()).unwrap();
        let role = U256::from_big_endian(&data[4..36]);
        let member = U256::from_big_endian(&data[36..68]);
        assert!(r["execution"]["reads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["key"] == w(proof::cases::position(role, member))));
    }
}
#[test]
fn gm_four_incoherent_routes_and_exact_revert_prefixes() {
    let role_id = role("INCOHERENT_ROLE");
    let key = w(proof::cases::member_key(role_id, 3.into()));
    for (name, sig, bool_change, set_change) in [
        ("incoherent_set_only_grant", "grantRole(bytes32,address)", false, true),
        ("incoherent_bool_only_grant", "grantRole(bytes32,address)", true, false),
        ("incoherent_bool_only_revoke", "revokeRole(bytes32,address)", true, false),
        ("incoherent_set_only_revoke", "revokeRole(bytes32,address)", false, true),
    ] {
        let r = record(name, sig);
        let writes = r["execution"]["writes"].as_array().unwrap();
        assert_eq!(writes.iter().any(|s| s["key"] == key), bool_change);
        assert_eq!(writes.iter().any(|s| s["key"] != key), set_change);
        assert_eq!(r["execution"]["logs"].as_array().unwrap().len(), usize::from(bool_change));
    }
    for (name, panic) in [
        ("empty_set_prefix_revert", 0x11u64),
        ("index_out_of_bounds_prefix_revert", 0x32),
        ("max_index_prefix_revert", 0x32),
    ] {
        let r = record(name, "revokeRole(bytes32,address)");
        let stores = r["execution"]["writes"].as_array().unwrap();
        let logs = r["execution"]["logs"].as_array().unwrap();
        assert_eq!(stores.len(), 1);
        assert_eq!(logs.len(), 1);
        assert_eq!(stores[0]["key"], key);
        assert_eq!(stores[0]["old"], w(1.into()));
        assert_eq!(stores[0]["new"], w(0.into()));
        assert_eq!(
            logs[0]["topics"],
            json!([w(role("RoleRevoked(bytes32,address,address)")), w(role_id), w(3.into()), w(1.into())])
        );
        assert_eq!(logs[0]["data"], "");
        assert!(stores[0]["step"].as_u64().unwrap() < logs[0]["step"].as_u64().unwrap());
        assert_eq!(r["execution"]["exit"]["data"], hex::encode(call("Panic(uint256)", &[panic.into()])));
    }
    let dirty = record("dirty_bool_high_bits", "revokeRole(bytes32,address)");
    assert_eq!(dirty["execution"]["writes"][0]["new"], w(U256::one() << 200));
    let wrap = record("incoherent_max_length_push", "grantRole(bytes32,address)");
    assert_eq!(wrap["execution"]["writes"][1]["old"], w(U256::MAX));
    assert_eq!(wrap["execution"]["writes"][1]["new"], w(0.into()));
}
#[test]
fn gm_initializer_order_and_early_late_rollback() {
    let sig = "initialize(string,string,address,address)";
    for name in ["initialize_short", "initialize_empty", "initialize_proxy_b", "initialize_constructor_context"] {
        let r = record(name, sig);
        let stores = r["execution"]["writes"].as_array().unwrap();
        let logs = r["execution"]["logs"].as_array().unwrap();
        assert_eq!(logs.len(), 4);
        assert_eq!(
            logs.iter().map(|l| l["topics"][0].clone()).collect::<Vec<_>>(),
            vec![
                json!(w(role("ComplianceSet(address,address)"))),
                json!(w(role("TokenPauseManagerSet(address,address)"))),
                json!(w(role("RoleGranted(bytes32,address,address)"))),
                json!(w(role("Initialized(uint8)")))
            ]
        );
        let compliance = stores.iter().find(|s| s["key"] == w(301.into())).unwrap();
        let manager = stores.iter().find(|s| s["key"] == w(351.into())).unwrap();
        assert!(compliance["step"].as_u64().unwrap() < logs[0]["step"].as_u64().unwrap());
        assert!(logs[1]["step"].as_u64().unwrap() < manager["step"].as_u64().unwrap());
        let s = state(&r["execution"]["committed_storage"]);
        assert_eq!(s[&proof::cases::member_key(0.into(), 9.into())], 1.into());
        assert_eq!(s[&0.into()], 1.into());
        assert_eq!(r["execution"]["committed_logs"], 4);
    }
    let allocation = record("initializer_max_dynamic_length", "malformed ABI");
    assert_eq!(allocation["execution"]["exit"]["data"], hex::encode(call("Panic(uint256)", &[0x41.into()])));
    for field in ["writes", "reads", "logs"] {
        assert_eq!(allocation["execution"][field], json!([]));
    }
    assert_eq!(record("initializer_bad_offset", "malformed ABI")["execution"]["exit"]["data"], "");
    let early = record("initialize_zero_compliance", sig);
    assert_eq!(early["execution"]["logs"], json!([]));
    assert!(!early["execution"]["writes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["key"] == w(301.into()) || s["key"] == w(351.into())));
    let late = record("initialize_zero_manager", sig);
    assert_eq!(late["execution"]["logs"].as_array().unwrap().len(), 1);
    assert_eq!(
        late["execution"]["logs"][0]["topics"],
        json!([w(role("ComplianceSet(address,address)")), w(0.into()), w(77.into())])
    );
    assert_eq!(late["execution"]["exit"]["data"], hex::encode(call("TokenPauseManagerCantBeZero()", &[])));
    assert_eq!(late["execution"]["committed_storage"], json!([]));
    assert!(!late["execution"]["writes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["key"] == w(351.into()) || s["key"] == w(401.into())));
    assert_eq!(record("initialize_constructor_context", sig)["self_code_size"], 0);
    assert_eq!(
        record("initialize_constructor_context_repeat_deployed", sig)["execution"]["exit"]["kind"],
        "revert"
    );
}
#[test]
fn gm_raw_getters_are_exact_single_reads_even_with_different_client_cells() {
    for r in records().iter().filter(|r| r["name"].as_str().unwrap().starts_with("raw_getter_")) {
        let data = hex::decode(r["calldata"].as_str().unwrap()).unwrap();
        let sig = r["signature"].as_str().unwrap();
        let key = match sig {
            "balanceOf(address)" => mapping(U256::from_big_endian(&data[4..36]), 51.into()),
            "totalSupply()" => 53.into(),
            "allowance(address,address)" => mapping(U256::from_big_endian(&data[36..68]), mapping(U256::from_big_endian(&data[4..36]), 52.into())),
            _ => panic!("unexpected getter"),
        };
        let pre = state(&r["prestate"]);
        assert_eq!(
            r["execution"]["exit"]["data"],
            hex::encode(proof::vm::word(pre.get(&key).copied().unwrap_or_default()))
        );
        assert_eq!(r["execution"]["reads"].as_array().unwrap().len(), 1);
        assert_eq!(r["execution"]["reads"][0]["key"], w(key));
    }
    assert_eq!(record("no_public_full_role_array", "malformed ABI")["execution"]["exit"]["data"], "");
}
#[test]
fn gm_unsupported_boundaries_do_not_mock_or_execute_clients() {
    for r in records().iter().filter(|r| r["scope"] == "unsupported_path_control") {
        let trace = r["execution"]["trace"].as_array().unwrap();
        let last = trace.last().unwrap();
        if r["name"] == "missing_self_context" {
            assert_eq!(last["pc"], 1889);
            assert_eq!(last["opcode"], 0x3b);
        } else {
            assert_eq!(last["pc"], 5556);
            assert_eq!(last["opcode"], 0x5a);
        }
        assert!(!trace.iter().any(|s| matches!(s["opcode"].as_u64().unwrap(), 0xf1 | 0xf4 | 0xfa)));
        assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
        assert_eq!(r["execution"]["committed_logs"], 0);
    }
    for (name, sig) in [
        ("excluded_transfer_from", "transferFrom(address,address,uint256)"),
        ("excluded_burn_from", "burnFrom(address,uint256)"),
    ] {
        let r = record(name, sig);
        assert_eq!(r["execution"]["writes"].as_array().unwrap().len(), 1);
        assert_eq!(r["execution"]["logs"][0]["topics"][0], w(role("Approval(address,address,uint256)")));
    }
}
#[test]
fn gm_corrupted_store_or_missing_log_fails_after_saving_without_panic() {
    let r = record("role_add_empty", "grantRole(bytes32,address)");
    let capture = proof::verify_capture(CAPTURE, &CAPTURES[0]).unwrap();
    for (field, opcode) in [("writes", 0x55u8), ("logs", 0xa4)] {
        let mut code = proof::runtime(&capture).unwrap();
        let pc = r["execution"][field][0]["pc"].as_u64().unwrap() as usize;
        assert_eq!(code[pc], opcode);
        code[pc] = 0x50;
        let mut p = Proof::new(&code);
        let mut attempted = vec![];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            p.matrix(&mut |_, v| {
                attempted.push(v.clone());
                Ok(())
            })
        }));
        assert!(result.is_ok(), "missing evidence must return a reportable error");
        assert!(result.unwrap().is_err());
        assert!(!attempted.is_empty());
        assert_eq!(attempted[0]["name"], "role_add_empty");
        assert!(attempted[0]["execution"]["trace"].is_array());
    }
}
