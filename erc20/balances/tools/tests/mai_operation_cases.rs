#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::mai_proof::{
    self as p,
    cases::{admin, balance, element, head, member, position, Proof},
};
use erc20_balances_tools::ptoken_proof::cases::{role, w};
use primitive_types::U256;
use serde_json::{json, Value};
use std::sync::OnceLock;
fn records() -> &'static [Value] {
    static R: OnceLock<Vec<Value>> = OnceLock::new();
    R.get_or_init(|| {
        let c = p::verify_capture(p::captured()).unwrap();
        let runtime = p::runtime(&c).unwrap();
        let mut proof = Proof::new(&runtime);
        let mut count = 0;
        let mut save = |n: usize, v: &Value| {
            assert_eq!(n, count + 1);
            assert!(v["execution"]["trace"].is_array());
            count = n;
            Ok(())
        };
        proof.constructor(&c, &mut save).unwrap();
        proof.matrix(&mut save).unwrap();
        assert_eq!(count, proof.calls);
        assert_eq!(count, proof.cases.len());
        assert!(count > 1000);
        proof.cases
    })
}
fn record(name: &str, sig: &str) -> &'static Value {
    records()
        .iter()
        .find(|v| v["name"] == name && v["signature"] == sig)
        .unwrap_or_else(|| panic!("missing {name}/{sig}"))
}
fn n(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}
fn writes(v: &Value) -> Vec<(U256, U256, U256)> {
    v["execution"]["writes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (n(&s["key"]), n(&s["old"]), n(&s["new"])))
        .collect()
}
fn kind(v: &Value) -> &str {
    v["execution"]["exit"]["kind"].as_str().unwrap()
}
#[test]
fn every_call_binds_exact_code_and_strict_exit_with_failure_rollback() {
    let c = p::verify_capture(p::captured()).unwrap();
    let runtime = p::runtime(&c).unwrap();
    let creation = p::bytes(&c["creationBytecode"]["onchainBytecode"]).unwrap();
    for v in records() {
        assert!(matches!(kind(v), "return" | "revert"));
        assert_eq!(v["address"], w(p::account()));
        assert_eq!(v["code_sha256"], p::sha(if v["code_kind"] == "bytecode" { &creation } else { &runtime }));
        if kind(v) == "revert" {
            assert_eq!(v["execution"]["committed_storage"], v["prestate"]);
            assert_eq!(v["execution"]["committed_logs"], 0);
        }
    }
}
#[test]
fn full_constructor_returns_patched_runtime_and_only_expected_roles() {
    let v = record("captured_no_argument_constructor", "constructor()");
    let c = p::verify_capture(p::captured()).unwrap();
    assert_eq!(kind(v), "return");
    assert_eq!(v["execution"]["exit"]["data"], hex::encode(p::runtime(&c).unwrap()));
    assert_eq!(v["prestate"], json!([]));
    let stores = writes(v);
    let caller = n(&v["caller"]);
    let mut keys = vec![5.into(), 6.into()];
    for r in [U256::zero(), role("MINTER_ROLE")] {
        keys.extend([member(r, caller), head(r), element(r, 0.into()), position(r, caller)]);
    }
    assert_eq!(stores.iter().map(|v| v.0).collect::<Vec<_>>(), keys);
    assert!(stores.iter().all(|v| v.1.is_zero()));
    assert_eq!(v["execution"]["logs"].as_array().unwrap().len(), 2);
    for (l, r) in v["execution"]["logs"].as_array().unwrap().iter().zip([U256::zero(), role("MINTER_ROLE")]) {
        assert_eq!(
            l["topics"],
            json!([w(role("RoleGranted(bytes32,address,address)")), w(r), w(caller), w(caller)])
        );
        assert_eq!(l["data"], "");
    }
}
#[test]
fn conditional_tail_removal_and_zero_equalities_are_exact() {
    let r = role("MATRIX_ROLE");
    let middle = record("remove_middle", "revokeRole(bytes32,address)");
    let tail = record("remove_tail", "revokeRole(bytes32,address)");
    assert_eq!(writes(middle).len(), 6);
    assert_eq!(
        writes(tail).iter().map(|v| v.0).collect::<Vec<_>>(),
        vec![member(r, 4.into()), element(r, 2.into()), head(r), position(r, 4.into())]
    );
    assert_eq!(
        writes(record("grant_only_zero", "grantRole(bytes32,address)"))[2],
        (element(r, 0.into()), 0.into(), 0.into())
    );
    assert_eq!(
        writes(record("remove_only_zero", "revokeRole(bytes32,address)"))[1],
        (element(r, 0.into()), 0.into(), 0.into())
    );
    for name in ["duplicate", "duplicate_zero", "absent_empty", "absent_nonempty"] {
        let sig = if name.starts_with("duplicate") {
            "grantRole(bytes32,address)"
        } else {
            "revokeRole(bytes32,address)"
        };
        let v = record(name, sig);
        assert!(writes(v).is_empty());
        assert_eq!(v["execution"]["logs"], json!([]));
        assert!(v["execution"]["reads"].as_array().unwrap().len() >= 4);
    }
}
#[test]
fn void_super_runs_the_set_even_when_membership_is_unchanged() {
    for (name, sig, stores, logs) in [
        ("grant_bool_only", "grantRole(bytes32,address)", 1, 1),
        ("grant_set_only", "grantRole(bytes32,address)", 3, 0),
        ("revoke_set_only", "revokeRole(bytes32,address)", 3, 0),
        ("revoke_bool_only", "revokeRole(bytes32,address)", 1, 1),
    ] {
        let v = record(name, sig);
        assert_eq!(kind(v), "return");
        assert_eq!(writes(v).len(), stores);
        assert_eq!(v["execution"]["logs"].as_array().unwrap().len(), logs);
    }
    for v in records().iter().filter(|v| v["name"] == "packed_boolean_padding") {
        let pre = v["prestate"].as_array().unwrap();
        let post = v["execution"]["committed_storage"].as_array().unwrap();
        let k = w(member(role("INCOHERENT"), 2.into()));
        let old = n(&pre.iter().find(|v| v["key"] == k).unwrap()["value"]);
        let new = n(&post.iter().find(|v| v["key"] == k).unwrap()["value"]);
        assert_eq!(old & !U256::from(255), new & !U256::from(255));
    }
}
#[test]
fn malformed_arrays_keep_explicit_wrap_dirty_tail_and_exact_panic_prefixes() {
    let v = records()
        .iter()
        .filter(|v| v["name"] == "malformed_length_push_source_semantics")
        .find(|v| writes(v)[1].1 == U256::max_value())
        .unwrap();
    assert_eq!(writes(v)[1].2, U256::zero());
    assert_eq!(writes(v).last().unwrap().2, U256::zero());
    for name in ["empty_length_position", "position_past_length"] {
        let v = record(name, "revokeRole(bytes32,address)");
        assert_eq!(kind(v), "revert");
        assert_eq!(writes(v).len(), 1);
        assert_eq!(v["execution"]["logs"].as_array().unwrap().len(), 1);
    }
    let v = record("dirty_tail_moves_full_bytes32_index", "revokeRole(bytes32,address)");
    assert_eq!(writes(v)[1].2, (U256::one() << 200) | U256::from(3u64));
    assert_eq!(writes(v)[2].0, position(role("MALFORMED_BOUNDARY"), (U256::one() << 200) | U256::from(3u64)));
}
#[test]
fn authorization_last_admin_abi_and_metadata_boundaries_are_measured() {
    assert_eq!(kind(record("last_admin_loss", "renounceRole(bytes32,address)")), "return");
    assert_eq!(kind(record("last_admin_cannot_regain", "grantRole(bytes32,address)")), "revert");
    assert_eq!(
        records()
            .iter()
            .filter(|v| v["name"] == "custom_full_width_admin" && kind(v) == "return")
            .count(),
        1
    );
    for name in ["unknown_selector", "empty_calldata", "truncated_calldata", "dirty_address"] {
        let v = record(name, "raw");
        assert_eq!(v["execution"]["exit"]["data"], "");
        assert!(writes(v).is_empty());
    }
    assert_eq!(kind(record("trailing_calldata", "raw")), "return");
    assert_eq!(kind(record("metadata", "name()")), "return");
    assert!(records().iter().filter(|v| v["name"] == "interfaces").count() >= 4);
    assert_eq!(
        admin(U256::zero()),
        erc20_balances_tools::ptoken_proof::cases::mapping(0.into(), 0.into())
            .overflowing_add(1.into())
            .0
    );
}
#[test]
fn mint_burn_and_late_arithmetic_failures_preserve_order_and_constant_cap() {
    let mint = record("mint_sequence", "mint(address,uint256)");
    assert_eq!(writes(mint).iter().map(|v| v.0).collect::<Vec<_>>(), vec![4.into(), balance(2.into())]);
    let burn = record("burn_sequence", "burn(uint256)");
    assert_eq!(writes(burn).iter().map(|v| v.0).collect::<Vec<_>>(), vec![balance(2.into()), 4.into()]);
    for (name, sig, count) in [
        ("transfer_credit_overflow", "transfer(address,uint256)", 1),
        ("transfer_from_late_overflow", "transferFrom(address,address,uint256)", 2),
        ("mint_credit_overflow", "mint(address,uint256)", 1),
        ("burn_supply_underflow", "burn(uint256)", 1),
        ("burn_from_supply_underflow", "burnFrom(address,uint256)", 2),
    ] {
        let v = record(name, sig);
        assert_eq!(kind(v), "revert");
        assert_eq!(writes(v).len(), count);
        assert_eq!(v["execution"]["committed_storage"], v["prestate"]);
    }
    assert!(records()
        .iter()
        .filter(|v| v["signature"] == "cap()")
        .all(|v| v["execution"]["exit"]["data"] == hex::encode(p::vm::word(U256::exp10(29)))));
}
#[test]
fn infinite_allowance_omits_approval_and_finite_paths_preserve_prefix() {
    let paths = records()
        .iter()
        .filter(|v| v["name"] == "transfer_from_allowance" && v["signature"] == "transferFrom(address,address,uint256)" && kind(v) == "return")
        .collect::<Vec<_>>();
    assert!(paths
        .iter()
        .any(|v| writes(v).len() == 2 && v["execution"]["logs"].as_array().unwrap().len() == 1));
    assert!(paths
        .iter()
        .any(|v| writes(v).len() == 3 && v["execution"]["logs"].as_array().unwrap().len() == 2));
    let v = record("burn_from_supply_underflow", "burnFrom(address,uint256)");
    assert_eq!(v["execution"]["logs"].as_array().unwrap().len(), 1);
    assert_eq!(v["execution"]["logs"][0]["topics"][0], w(role("Approval(address,address,uint256)")));
}
#[test]
fn corrupt_runtime_and_invalid_exit_are_saved_before_validation_failure() {
    for code in [vec![0x00], vec![0xfe]] {
        let mut p = Proof::new(&code);
        let mut saved = vec![];
        let result = p.matrix(&mut |_, v| {
            saved.push(v.clone());
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(saved.len(), 1);
        assert!(saved[0]["execution"]["trace"].is_array());
        assert_eq!(saved[0]["code_sha256"], p::sha(&code));
    }
}
