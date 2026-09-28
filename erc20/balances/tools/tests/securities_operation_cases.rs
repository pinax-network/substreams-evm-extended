#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::ptoken_proof::cases::{call, role, w};
use erc20_balances_tools::securities_proof::{
    self as proof,
    cases::{self, Proof},
    vm,
};
use primitive_types::U256;
use serde_json::Value;
use std::sync::OnceLock;
const CAPTURE: &[u8] = include_bytes!("../../tests/fixtures/securities-operation-proof/capture.json");
fn records() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let c = proof::verify_capture(CAPTURE).unwrap();
        assert!(c["creationBytecode"]["onchainBytecode"].is_null());
        assert!(c["creationMatch"].is_null());
        let runtime = proof::runtime(&c).unwrap();
        let mut p = Proof::new(&runtime);
        let mut attempted = 0;
        let mut save = |n: usize, value: &Value| {
            assert_eq!(n, attempted + 1);
            attempted = n;
            assert!(value["execution"]["trace"].as_array().is_some());
            Ok(())
        };
        p.constructor(&proof::bytes(&c["creationBytecode"]["recompiledBytecode"]).unwrap(), &mut save)
            .unwrap();
        p.matrix(&mut save).unwrap();
        assert_eq!(p.calls, attempted);
        assert_eq!(p.cases.len(), attempted);
        p.cases
    })
}
fn row(name: &str, sig: &str) -> &'static Value {
    records()
        .iter()
        .find(|r| r["name"] == name && r["signature"] == sig)
        .unwrap_or_else(|| panic!("missing {name}:{sig}"))
}
fn word(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().strip_prefix("0x").unwrap(), 16).unwrap()
}
fn state(v: &Value) -> vm::State {
    v.as_array().unwrap().iter().map(|v| (word(&v["key"]), word(&v["value"]))).collect()
}
#[test]
fn securities_complete_actual_runtime_local_matrix() {
    assert!(records().len() > 500);
    let contract = U256::from_big_endian(&hex::decode(&proof::CONTRACT[2..]).unwrap());
    for r in records() {
        assert_eq!(r["address"], w(contract));
        assert_eq!(r["self_code_size"], if r["name"] == "synthetic_implementation_constructor" { 0 } else { 10836 });
        if r["scope"] != "unsupported_path_control" {
            assert!(matches!(r["execution"]["exit"]["kind"].as_str(), Some("return" | "revert")));
        }
        if r["execution"]["exit"]["kind"] == "revert" || r["execution"]["exit"]["kind"] == "harness_failure" {
            assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
            assert_eq!(r["execution"]["committed_logs"], 0);
        }
    }
    let ctor = row("synthetic_implementation_constructor", "constructor()");
    assert_eq!(ctor["scope"], "synthetic_constructor_only");
    assert_eq!(
        state(&ctor["execution"]["committed_storage"]),
        vm::State::from([(cases::initializer_slot(), u64::MAX.into())])
    );
}
#[test]
fn securities_namespace_derivation_and_raw_getters_are_independent_of_clients_and_ui() {
    for (label, expected) in [
        ("openzeppelin.storage.AccessControl", cases::role_root()),
        ("openzeppelin.storage.AccessControlEnumerable", cases::set_root()),
        ("openzeppelin.storage.ERC20", cases::erc20_root()),
        ("openzeppelin.storage.Initializable", cases::initializer_slot()),
    ] {
        let seed = vm::hash(label.as_bytes()) - U256::one();
        let derived = vm::hash(&vm::word(seed)) & !U256::from(255);
        assert_eq!(derived, expected);
    }
    for r in records() {
        let sig = r["signature"].as_str().unwrap();
        if !matches!(sig, "balanceOf(address)" | "allowance(address,address)" | "totalSupply()") {
            continue;
        }
        let data = hex::decode(r["calldata"].as_str().unwrap()).unwrap();
        let key = match sig {
            "balanceOf(address)" => cases::balance_key(U256::from_big_endian(&data[4..36])),
            "allowance(address,address)" => cases::allowance_key(U256::from_big_endian(&data[4..36]), U256::from_big_endian(&data[36..68])),
            _ => cases::erc20_root() + U256::from(2),
        };
        let s = state(&r["prestate"]);
        let expected = s.get(&key).copied().unwrap_or_default();
        assert_eq!(r["execution"]["exit"]["data"], hex::encode(vm::word(expected)));
        let reads = r["execution"]["reads"].as_array().unwrap();
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0]["key"], w(key));
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
        assert!(r["execution"]["logs"].as_array().unwrap().is_empty());
    }
}
#[test]
fn securities_physical_equality_stores_and_array_readback() {
    for (name, sig) in [
        ("grant_zero_empty", "grantRole(bytes32,address)"),
        ("revoke_zero_sole", "revokeRole(bytes32,address)"),
        ("revoke_zero_tail_swap", "revokeRole(bytes32,address)"),
    ] {
        let effects = row(name, sig)["execution"]["writes"].as_array().unwrap();
        assert!(effects.iter().any(|e| word(&e["old"]).is_zero() && word(&e["new"]).is_zero()), "{name}");
    }
    for (name, sig) in [
        ("grant_duplicate", "grantRole(bytes32,address)"),
        ("grant_duplicate_zero", "grantRole(bytes32,address)"),
        ("revoke_absent", "revokeRole(bytes32,address)"),
        ("renounce_absent", "renounceRole(bytes32,address)"),
    ] {
        let r = row(name, sig);
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
        assert!(r["execution"]["logs"].as_array().unwrap().is_empty());
    }
    let data = hex::decode(row("revoke_first", "getRoleMembers(bytes32)")["execution"]["exit"]["data"].as_str().unwrap()).unwrap();
    assert_eq!(
        data.chunks_exact(32).map(U256::from_big_endian).collect::<Vec<_>>(),
        vec![32.into(), 2.into(), 5.into(), 4.into()]
    );
    let tail = row("revoke_tail", "revokeRole(bytes32,address)")["execution"]["writes"].as_array().unwrap();
    assert_eq!(tail.len(), 4, "tail skips swap stores");
}
#[test]
fn securities_exact_reverted_bool_log_prefix_and_oz53_gate() {
    let role_id = role("MALFORMED_ROLE");
    let bool_key = w(cases::member_key(role_id, 3.into()));
    for name in ["empty_array_prefix_revert", "out_of_bounds_prefix_revert", "max_index_prefix_revert"] {
        let r = row(name, "revokeRole(bytes32,address)");
        let e = &r["execution"];
        let writes = e["writes"].as_array().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0]["key"], bool_key);
        assert_eq!(writes[0]["old"], w(1.into()));
        assert_eq!(writes[0]["new"], w(0.into()));
        let logs = e["logs"].as_array().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(
            logs[0]["topics"],
            serde_json::json!([w(role("RoleRevoked(bytes32,address,address)")), w(role_id), w(3.into()), w(1.into())])
        );
        assert_eq!(logs[0]["data"], "");
        assert!(writes[0]["step"].as_u64().unwrap() < logs[0]["step"].as_u64().unwrap());
        assert_eq!(
            e["exit"]["data"],
            hex::encode(call(
                "Panic(uint256)",
                &[if name == "empty_array_prefix_revert" { 0x11.into() } else { 0x32.into() }]
            ))
        );
        assert_eq!(e["committed_logs"], 0);
    }
    for (name, sig, changed) in [
        ("malformed_true_no_index_grant", "grantRole(bytes32,address)", false),
        ("malformed_false_index_grant", "grantRole(bytes32,address)", true),
        ("malformed_true_no_index_revoke", "revokeRole(bytes32,address)", true),
        ("malformed_false_index_revoke", "revokeRole(bytes32,address)", false),
    ] {
        let e = &row(name, sig)["execution"];
        let writes = e["writes"].as_array().unwrap();
        assert_eq!(writes.len(), usize::from(changed));
        assert_eq!(e["logs"].as_array().unwrap().len(), usize::from(changed));
        if changed {
            assert_eq!(writes[0]["key"], bool_key);
        }
    }
    let moved = row("moved_tail_false_bool_outside_domain", "revokeRole(bytes32,address)");
    assert!(!moved["execution"]["reads"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["key"] == w(cases::member_key(role_id, 4.into()))));
}
#[test]
fn securities_arbitrary_admin_identity_and_no_public_admin_writer() {
    for (name, admin) in [
        ("full_width_admin", U256::max_value()),
        ("self_admin", role("ARBITRARY_ADMIN_TARGET")),
        ("high_bit_admin", U256::one() << 255),
    ] {
        let r = row(name, "grantRole(bytes32,address)");
        assert_eq!(r["caller"], w(7.into()));
        let key = cases::admin_key(role("ARBITRARY_ADMIN_TARGET"));
        assert_eq!(state(&r["prestate"])[&key], admin);
        assert_eq!(state(&r["execution"]["committed_storage"])[&key], admin);
        assert!(!r["execution"]["writes"].as_array().unwrap().iter().any(|v| v["key"] == w(key)));
        let denied = row(&format!("{name}_denied_grant"), "grantRole(bytes32,address)");
        assert_eq!(
            denied["execution"]["exit"]["data"],
            hex::encode(call("AccessControlUnauthorizedAccount(address,bytes32)", &[1.into(), admin]))
        );
    }
}
#[test]
fn securities_unsupported_paths_never_become_client_success_or_solidity_revert() {
    let controls: Vec<_> = records().iter().filter(|r| r["scope"] == "unsupported_path_control").collect();
    assert_eq!(controls.len(), 8);
    for r in controls {
        assert_eq!(r["execution"]["exit"]["kind"], "harness_failure");
        assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
        assert_eq!(r["execution"]["committed_logs"], 0);
        let name = r["name"].as_str().unwrap();
        let expected = if name == "excluded_set_compliance" || name == "excluded_set_pause_manager" {
            0x3b
        } else if matches!(name, "excluded_transfer" | "excluded_mint" | "excluded_burn") {
            // Stop at GAS before STATICCALL; no client call was executed.
            let trace = r["execution"]["trace"].as_array().unwrap();
            assert_eq!(trace.last().unwrap()["pc"], 7368);
            assert!(!trace.iter().any(|s| matches!(s["opcode"].as_u64(), Some(0xf1 | 0xf4 | 0xfa))));
            0x5a
        } else {
            0x42
        };
        assert_eq!(r["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], expected);
    }
}
#[test]
fn securities_mutated_write_or_log_returns_error_after_raw_attempt_without_panicking() {
    let normal = row("grant_empty", "grantRole(bytes32,address)");
    let c = proof::verify_capture(CAPTURE).unwrap();
    let original = proof::runtime(&c).unwrap();
    for (field, opcode) in [("writes", 0x55u8), ("logs", 0xa4)] {
        let pc = normal["execution"][field][0]["pc"].as_u64().unwrap() as usize;
        let mut changed = original.clone();
        assert_eq!(changed[pc], opcode);
        changed[pc] = 0x50;
        let mut p = Proof::new(&changed);
        let mut attempts = vec![];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            p.matrix(&mut |_, v| {
                attempts.push(v.clone());
                Ok(())
            })
        }));
        assert!(result.is_ok(), "malformed execution must return a diagnostic, not panic");
        assert!(result.unwrap().is_err());
        assert!(!attempts.is_empty());
        assert_eq!(attempts[0]["name"], "grant_empty");
        assert!(attempts[0]["execution"]["trace"].as_array().is_some());
    }
}
