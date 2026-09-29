#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::erc20tokenx_proof::{
    self as proof,
    cases::{admin, element, head, position, Proof},
};
use erc20_balances_tools::ptoken_proof::cases::{call, mapping, role, w};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{path::Path, sync::OnceLock};
const ORI: &[u8] = include_bytes!("../../tests/fixtures/erc20tokenx-operation-proof/ori-capture.json");
const FNA: &[u8] = include_bytes!("../../tests/fixtures/erc20tokenx-operation-proof/fna-capture.json");
fn records() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let a = proof::verify_capture(ORI, 0).unwrap();
        let b = proof::verify_capture(FNA, 1).unwrap();
        let compiler = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/erc20tokenx-operation-proof/compiler-output.json")).unwrap();
        proof::verify_compiled(&a, &compiler).unwrap();
        proof::verify_compiled(&b, &compiler).unwrap();
        let code = proof::runtime(&a).unwrap();
        assert_eq!(code, proof::runtime(&b).unwrap());
        let mut p = Proof::new(&code);
        let mut count = 0;
        let mut save = |n: usize, v: &Value| {
            assert_eq!(n, count + 1);
            assert!(v["execution"]["trace"].is_array());
            count = n;
            Ok(())
        };
        p.constructor(&a, &mut save).unwrap();
        p.constructor(&b, &mut save).unwrap();
        p.matrix(&mut save).unwrap();
        assert_eq!(p.calls, count);
        assert_eq!(p.calls, p.cases.len());
        p.cases
    })
}
fn record(name: &str, signature: &str) -> &'static Value {
    records()
        .iter()
        .find(|r| r["name"] == name && r["signature"] == signature)
        .unwrap_or_else(|| panic!("missing {name}: {signature}"))
}
fn n(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}
fn state(v: &Value) -> proof::vm::State {
    v.as_array().unwrap().iter().map(|x| (n(&x["key"]), n(&x["value"]))).collect()
}

#[test]
fn tokenx_complete_matrix_preserves_exact_failure_classes() {
    assert!(records().len() > 400);
    for r in records() {
        let kind = r["execution"]["exit"]["kind"].as_str().unwrap();
        assert!(matches!(kind, "return" | "revert" | "invalid" | "harness_failure"));
        assert_eq!(kind == "invalid", r["scope"] == "source_invalid_control");
        assert_eq!(kind == "harness_failure", r["scope"] == "unsupported_path_control");
        if kind != "return" {
            assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
            assert_eq!(r["execution"]["committed_logs"], 0);
        }
    }
    for contract in [proof::ORI, proof::FNA, proof::PHI] {
        assert!(records()
            .iter()
            .any(|r| r["name"] == "runtime_identity_raw_getter" && r["address"] == w(proof::address(contract))));
    }
}
#[test]
fn tokenx_captured_constructors_stop_before_domain_and_roles() {
    for (label, name, symbol) in [("ori", "Orizon", "ORI"), ("fna", "FinTech AI", "FNA")] {
        let r = record(&format!("{label}_constructor"), "constructor(address,uint256,uint256,string,string)");
        assert_eq!(r["code_kind"], "bytecode");
        assert_eq!(r["self_code_size"], 0);
        let writes = r["execution"]["writes"].as_array().unwrap();
        assert_eq!(writes.len(), 3);
        for (i, s) in [name, symbol].into_iter().enumerate() {
            let mut expected = [0u8; 32];
            expected[..s.len()].copy_from_slice(s.as_bytes());
            expected[31] = (2 * s.len()) as u8;
            assert_eq!(writes[i]["key"], w((i + 3).into()));
            assert_eq!(writes[i]["old"], w(0.into()));
            assert_eq!(writes[i]["new"], w(U256::from_big_endian(&expected)));
        }
        assert_eq!(writes[2]["key"], w(5.into()));
        assert_eq!(writes[2]["new"], w(9.into()));
        assert_eq!(r["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0x46);
        assert_eq!(r["execution"]["trace"].as_array().unwrap().last().unwrap()["pc"], 509);
        assert_eq!(r["execution"]["logs"], json!([]));
        assert_eq!(r["execution"]["committed_storage"], json!([]));
    }
    assert_eq!(records().iter().filter(|r| r["code_kind"] == "bytecode").count(), 2);
}
#[test]
fn tokenx_legacy_self_swap_equalities_and_noops_are_physical() {
    let r = role("TEST_ROLE");
    for (name, member) in [
        ("role_remove_tail", 5),
        ("role_remove_sole", 3),
        ("role_remove_zero_sole", 0),
        ("role_remove_zero_tail", 0),
    ] {
        let c = record(name, "revokeRole(bytes32,address)");
        let writes = c["execution"]["writes"].as_array().unwrap();
        assert_eq!(writes.len(), 5);
        assert_eq!(
            writes.iter().map(|s| s["pc"].as_u64().unwrap()).collect::<Vec<_>>(),
            [7072, 7092, 7123, 7125, 7150]
        );
        assert_eq!(writes[0]["old"], writes[0]["new"]);
        assert_eq!(writes[1]["old"], writes[1]["new"]);
        assert_eq!(writes[1]["key"], w(position(r, member.into())));
        assert_eq!(writes[4]["key"], w(position(r, member.into())));
        assert_eq!(writes[4]["new"], w(0.into()));
        assert_eq!(
            c["execution"]["logs"][0]["topics"],
            json!([w(role("RoleRevoked(bytes32,address,address)")), w(r), w(member.into()), w(1.into())])
        );
    }
    for (name, sig) in [
        ("role_add_zero_empty", "grantRole(bytes32,address)"),
        ("role_remove_zero_sole", "revokeRole(bytes32,address)"),
        ("role_move_zero_tail", "revokeRole(bytes32,address)"),
    ] {
        assert!(record(name, sig)["execution"]["writes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| n(&s["old"]).is_zero() && n(&s["new"]).is_zero()));
    }
    for (name, sig) in [
        ("role_duplicate", "grantRole(bytes32,address)"),
        ("role_duplicate_zero", "grantRole(bytes32,address)"),
        ("role_absent_empty", "revokeRole(bytes32,address)"),
        ("role_absent_nonempty", "revokeRole(bytes32,address)"),
        ("renounce_absent", "renounceRole(bytes32,address)"),
    ] {
        let c = record(name, sig);
        assert_eq!(c["execution"]["writes"], json!([]));
        assert_eq!(c["execution"]["logs"], json!([]));
        assert!(!c["execution"]["reads"].as_array().unwrap().is_empty());
    }
}
#[test]
fn tokenx_malformed_state_bounds_are_not_modern_panics_or_admission() {
    for name in ["empty_nonzero_position", "out_of_bounds_position", "max_position"] {
        let r = record(name, "revokeRole(bytes32,address)");
        assert_eq!(r["scope"], "source_invalid_control");
        assert_eq!(r["execution"]["exit"]["kind"], "invalid");
        assert_eq!(r["execution"]["writes"], json!([]));
        assert_eq!(r["execution"]["logs"], json!([]));
        assert_eq!(r["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0xfe);
    }
    let c = record("malformed_max_length_push", "grantRole(bytes32,address)");
    let r = role("MALFORMED_ROLE");
    assert_eq!(c["execution"]["writes"][0]["key"], w(head(r)));
    assert_eq!(c["execution"]["writes"][0]["old"], w(U256::MAX));
    assert_eq!(c["execution"]["writes"][0]["new"], w(0.into()));
    assert_eq!(c["execution"]["writes"][1]["key"], w(element(r, U256::MAX)));
    assert_eq!(c["execution"]["writes"][2]["new"], w(0.into()));
    // User-visible _at has an explicit require and differs from internal INVALID.
    let errors: Vec<_> = records()
        .iter()
        .filter(|r| r["signature"] == "getRoleMember(bytes32,uint256)" && r["execution"]["exit"]["kind"] == "revert")
        .collect();
    assert!(!errors.is_empty());
    for r in errors {
        assert!(r["execution"]["exit"]["data"].as_str().unwrap().starts_with("08c379a0"));
    }
}
#[test]
fn tokenx_authorization_and_raw_getters_do_not_mutate_sentinels() {
    let r = role("TEST_ROLE");
    let c = record("custom_admin_authorized", "grantRole(bytes32,address)");
    assert_eq!(state(&c["execution"]["committed_storage"])[&admin(r)], role("CUSTOM_ADMIN"));
    for (name, sig) in [
        ("wrong_grant_admin", "grantRole(bytes32,address)"),
        ("wrong_revoke_admin", "revokeRole(bytes32,address)"),
        ("wrong_renounce_account", "renounceRole(bytes32,address)"),
    ] {
        let c = record(name, sig);
        assert_eq!(c["execution"]["exit"]["kind"], "revert");
        assert_eq!(c["execution"]["writes"], json!([]));
        assert_eq!(c["execution"]["logs"], json!([]));
    }
    for c in records().iter().filter(|r| r["name"].as_str().unwrap().starts_with("raw_getter_")) {
        let data = hex::decode(c["calldata"].as_str().unwrap()).unwrap();
        let sig = c["signature"].as_str().unwrap();
        let key = match sig {
            "balanceOf(address)" => mapping(U256::from_big_endian(&data[4..36]), 0.into()),
            "totalSupply()" => 2.into(),
            "allowance(address,address)" => mapping(U256::from_big_endian(&data[36..68]), mapping(U256::from_big_endian(&data[4..36]), 1.into())),
            "nonces(address)" => mapping(U256::from_big_endian(&data[4..36]), 6.into()),
            _ => panic!("getter"),
        };
        assert_eq!(c["execution"]["reads"].as_array().unwrap().len(), 1);
        assert_eq!(c["execution"]["reads"][0]["key"], w(key));
        assert_eq!(c["execution"]["exit"]["data"], hex::encode(proof::vm::word(state(&c["prestate"])[&key])));
    }
}
#[test]
fn tokenx_excluded_external_fee_and_permit_paths_preserve_prefix_and_rollback() {
    let fee = record("excluded_fee_receiver", "transfer(address,uint256)");
    assert_eq!(fee["execution"]["writes"].as_array().unwrap().len(), 2);
    assert_eq!(fee["execution"]["logs"].as_array().unwrap().len(), 2);
    assert_eq!(fee["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0x3b);
    assert_eq!(fee["execution"]["trace"].as_array().unwrap().last().unwrap()["pc"], 5169);
    assert_eq!(fee["execution"]["exit"]["error"], "EXTCODESIZE unknown external account");
    let permit = record("excluded_permit", "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)");
    assert_eq!(permit["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0x42);
    assert_eq!(permit["execution"]["trace"].as_array().unwrap().last().unwrap()["pc"], 3331);
    assert_eq!(permit["execution"]["writes"], json!([]));
    for c in [fee, permit] {
        assert_eq!(c["scope"], "unsupported_path_control");
        assert_eq!(c["execution"]["committed_storage"], c["prestate"]);
        assert_eq!(c["execution"]["committed_logs"], 0);
    }
}
#[test]
fn tokenx_corrupted_store_log_and_unexpected_invalid_are_saved_reportable_failures() {
    let c = proof::verify_capture(ORI, 0).unwrap();
    let first = record("role_add_empty", "grantRole(bytes32,address)");
    assert_eq!(
        first["execution"]["writes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["pc"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [6309, 6325, 6344]
    );
    for (field, replacement) in [("writes", 0x50), ("logs", 0x50), ("writes", 0xfe)] {
        let mut code = proof::runtime(&c).unwrap();
        let pc = first["execution"][field][0]["pc"].as_u64().unwrap() as usize;
        code[pc] = replacement;
        let mut p = Proof::new(&code);
        let mut attempts = vec![];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            p.matrix(&mut |_, v| {
                attempts.push(v.clone());
                Ok(())
            })
        }));
        assert!(result.is_ok(), "negative must return error, not panic");
        assert!(result.unwrap().is_err());
        assert!(!attempts.is_empty());
        assert_eq!(attempts[0]["name"], "role_add_empty");
        assert!(attempts[0]["execution"]["trace"].is_array());
    }
    let dirty = record("dirty_address_abi", "grantRole(bytes32,address)");
    assert_eq!(dirty["execution"]["logs"][0]["topics"][2], w(3.into()));
    assert_eq!(
        record("no_public_admin_setter", "setRoleAdmin(bytes32,bytes32)")["execution"]["exit"]["data"],
        ""
    );
    assert_eq!(call("grantRole(bytes32,address)", &[role("TEST_ROLE"), 3.into()]).len(), 68);
}
#[test]
fn tokenx_full_width_admin_and_last_admin_loss_are_stateful() {
    let role_id = role("TEST_ROLE");
    let granted = record("max_admin_authorized", "grantRole(bytes32,address)");
    assert_eq!(state(&granted["prestate"])[&admin(role_id)], U256::MAX);
    assert_eq!(state(&granted["execution"]["committed_storage"])[&admin(role_id)], U256::MAX);
    assert_eq!(granted["execution"]["logs"][0]["topics"][3], w(2.into()));
    let lost = record("last_admin_renounce", "renounceRole(bytes32,address)");
    let no_admin = state(&lost["execution"]["committed_storage"]);
    assert_eq!(no_admin[&head(0.into())], U256::zero());
    assert_eq!(no_admin[&position(0.into(), 1.into())], U256::zero());
    let denied = record("grant_after_last_admin_renounce", "grantRole(bytes32,address)");
    assert_eq!(state(&denied["prestate"]), no_admin);
    assert_eq!(denied["execution"]["exit"]["kind"], "revert");
    assert_eq!(denied["execution"]["writes"], json!([]));
    assert_eq!(denied["execution"]["logs"], json!([]));
    let repeated = record("repeat_last_admin_renounce", "renounceRole(bytes32,address)");
    assert_eq!(repeated["execution"]["exit"]["kind"], "return");
    assert_eq!(repeated["execution"]["writes"], json!([]));
    assert_eq!(repeated["execution"]["logs"], json!([]));
    let fee = state(&record("excluded_fee_receiver", "transfer(address,uint256)")["prestate"]);
    assert_eq!(fee[&2.into()], fee[&mapping(3.into(), 0.into())] + fee[&mapping(44.into(), 0.into())]);
}
