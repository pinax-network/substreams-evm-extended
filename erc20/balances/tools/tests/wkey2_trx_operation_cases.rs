#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{
    ptoken_proof::cases::{mapping, role, w},
    wkey2_trx_proof::{
        self as proof,
        cases::{admin, element, head, position, Proof},
        Target,
    },
};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};
fn records(t: Target) -> &'static [Value] {
    static ALL: OnceLock<BTreeMap<&'static str, Vec<Value>>> = OnceLock::new();
    &ALL.get_or_init(|| {
        let mut all = BTreeMap::new();
        for t in Target::ALL {
            let c = proof::verify_capture(t.captured(), t).unwrap();
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/wkey2-trx-operation-proof")
                .join(t.label())
                .join("compiler-output.json");
            proof::verify_compiled(&c, &std::fs::read(p).unwrap(), t).unwrap();
            let compiled = proof::bytes(&c["runtimeBytecode"]["recompiledBytecode"]).unwrap();
            let captured = proof::runtime(&c).unwrap();
            let append = proof::arguments(t);
            let compiler_creation = [proof::bytes(&c["creationBytecode"]["recompiledBytecode"]).unwrap(), append].concat();
            let saved_creation = proof::bytes(&c["creationBytecode"]["onchainBytecode"]).unwrap();
            let mut compiled_cases = None;
            for (code, creation) in [(&compiled, &compiler_creation), (&captured, &saved_creation)] {
                let mut p = Proof::new(code, t);
                let mut saved = 0;
                let mut save = |n: usize, v: &Value| {
                    assert_eq!(n, saved + 1);
                    assert!(v["execution"]["trace"].is_array());
                    saved = n;
                    Ok(())
                };
                p.constructor(&c, creation, &mut save).unwrap();
                p.matrix(&mut save).unwrap();
                assert_eq!(p.calls, saved);
                assert_eq!(p.calls, p.cases.len());
                for r in &p.cases {
                    proof::verify_trace(&r["execution"], t, r["code_kind"] == "bytecode").unwrap();
                }
                let comparison: Vec<_> = p
                    .cases
                    .iter()
                    .cloned()
                    .map(|mut v| {
                        v.as_object_mut().unwrap().remove("code_sha256");
                        v.as_object_mut().unwrap().remove("constructor_code");
                        v
                    })
                    .collect();
                if let Some(expected) = compiled_cases.take() {
                    assert_eq!(comparison, expected, "entire compiled/captured trace and state identity for {}", t.label());
                    all.insert(t.label(), p.cases);
                } else {
                    compiled_cases = Some(comparison);
                }
            }
        }
        all
    })[t.label()]
}
fn record(t: Target, name: &str, sig: &str) -> &'static Value {
    let found: Vec<_> = records(t).iter().filter(|r| r["name"] == name && r["signature"] == sig).collect();
    assert_eq!(found.len(), 1, "unique {name}/{sig}");
    found[0]
}
fn num(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}
fn state(v: &Value) -> proof::vm::State {
    v.as_array().unwrap().iter().map(|r| (num(&r["key"]), num(&r["value"]))).collect()
}
fn constructor(t: Target) -> &'static Value {
    record(
        t,
        "constructor",
        if t == Target::Trx {
            "constructor(string,string)"
        } else {
            "constructor(address,address,uint256)"
        },
    )
}
#[test]
fn both_compiled_and_captured_complete_matrices_are_identical_and_failures_are_bounded() {
    for t in Target::ALL {
        assert!(records(t).len() > 500);
        for r in records(t) {
            let kind = r["execution"]["exit"]["kind"].as_str().unwrap();
            assert!(matches!(kind, "return" | "revert" | "invalid" | "harness_failure"));
            assert_eq!(kind == "invalid", r["scope"] == "source_invalid_control");
            assert_eq!(kind == "harness_failure", r["scope"] == "unsupported_path_control");
            if kind != "return" {
                assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
                assert_eq!(r["execution"]["committed_logs"], 0);
            }
        }
    }
}
#[test]
fn constructor_prefixes_are_distinct_and_do_not_commit_initialization() {
    for t in Target::ALL {
        let r = constructor(t);
        let writes = r["execution"]["writes"].as_array().unwrap();
        assert_eq!(r["self_code_size"], 0);
        assert_eq!(r["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0x46);
        assert_eq!(r["execution"]["committed_storage"], json!([]));
        let keys: Vec<_> = writes.iter().map(|x| num(&x["key"])).collect();
        if t == Target::Wkeydao2 {
            assert_eq!(keys, [12.into(), 3.into(), 4.into(), 5.into()]);
            assert_eq!(writes[0]["new"], w(4000.into()));
            assert_eq!(writes[3]["new"], w(9.into()));
            assert_eq!(r["execution"]["logs"], json!([]));
        } else {
            let pr = role("PREDICATE_ROLE");
            let member = proof::address("0xca266910d92a313e5f9eb1affc462bcbb7d9c4a9");
            assert_eq!(
                keys,
                [
                    8.into(),
                    3.into(),
                    4.into(),
                    5.into(),
                    7.into(),
                    head(t, pr),
                    element(t, pr, 0.into()),
                    position(t, pr, member)
                ]
            );
            assert_eq!(writes[0]["old"], writes[0]["new"]);
            assert_eq!(writes[3]["new"], w(6.into()));
            let logs = r["execution"]["logs"].as_array().unwrap();
            assert_eq!(logs.len(), 1);
            assert_eq!(
                logs[0]["topics"],
                json!([w(role("RoleGranted(bytes32,address,address)")), w(pr), w(member), r["caller"]])
            );
            assert!(!keys.contains(&head(t, 0.into())));
            assert!(!keys.contains(&U256::from(9)));
        }
    }
}
#[test]
fn legacy_self_swaps_zero_equalities_and_noops_are_measured_per_target() {
    for t in Target::ALL {
        let r = role("TEST_ROLE");
        for (name, m) in [
            ("role_remove_tail", 5),
            ("role_remove_sole", 3),
            ("role_remove_zero_sole", 0),
            ("role_remove_zero_tail", 0),
        ] {
            let c = record(t, name, "revokeRole(bytes32,address)");
            let writes = c["execution"]["writes"].as_array().unwrap();
            assert_eq!(writes.len(), 5);
            assert_eq!(writes[0]["old"], writes[0]["new"]);
            assert_eq!(writes[1]["old"], writes[1]["new"]);
            assert_eq!(writes[1]["key"], w(position(t, r, m.into())));
            assert_eq!(writes[4]["key"], w(position(t, r, m.into())));
            assert_eq!(writes[4]["new"], w(0.into()));
        }
        let add = record(t, "role_add_zero_empty", "grantRole(bytes32,address)");
        assert_eq!(add["execution"]["writes"].as_array().unwrap().len(), 3);
        assert_eq!(add["execution"]["writes"][1]["old"], add["execution"]["writes"][1]["new"]);
        for (name, sig) in [
            ("role_duplicate", "grantRole(bytes32,address)"),
            ("role_duplicate_zero", "grantRole(bytes32,address)"),
            ("role_absent_empty", "revokeRole(bytes32,address)"),
            ("renounce_absent", "renounceRole(bytes32,address)"),
        ] {
            let c = record(t, name, sig);
            assert_eq!(c["execution"]["writes"], json!([]));
            assert_eq!(c["execution"]["logs"], json!([]));
            assert!(!c["execution"]["reads"].as_array().unwrap().is_empty());
        }
    }
}
#[test]
fn source_malformed_states_and_authorization_remain_separate_from_coherent_sets() {
    for t in Target::ALL {
        for name in ["empty_nonzero_position", "out_of_bounds_position", "max_position"] {
            let c = record(t, name, "revokeRole(bytes32,address)");
            assert_eq!(c["execution"]["exit"]["kind"], "invalid");
            assert_eq!(c["execution"]["writes"], json!([]));
            assert_eq!(c["execution"]["logs"], json!([]));
        }
        let c = record(t, "malformed_max_length_push", "grantRole(bytes32,address)");
        assert_eq!(c["execution"]["writes"][0]["old"], w(U256::MAX));
        assert_eq!(c["execution"]["writes"][0]["new"], w(0.into()));
        for (name, sig) in [
            ("wrong_grant_admin", "grantRole(bytes32,address)"),
            ("wrong_revoke_admin", "revokeRole(bytes32,address)"),
            ("wrong_renounce_account", "renounceRole(bytes32,address)"),
        ] {
            let c = record(t, name, sig);
            assert_eq!(c["execution"]["exit"]["kind"], "revert");
            assert_eq!(c["execution"]["writes"], json!([]));
            assert_eq!(c["execution"]["logs"], json!([]));
        }
        let max = record(t, "max_admin_authorized", "grantRole(bytes32,address)");
        assert_eq!(state(&max["execution"]["committed_storage"])[&admin(t, role("TEST_ROLE"))], U256::MAX);
    }
}
#[test]
fn raw_full_width_values_and_address_masking_have_exact_single_reads() {
    for t in Target::ALL {
        for c in records(t).iter().filter(|c| c["name"].as_str().unwrap().starts_with("raw_getter_")) {
            let reads = c["execution"]["reads"].as_array().unwrap();
            assert_eq!(reads.len(), 1);
            let v = num(&reads[0]["value"]);
            let want = if matches!(c["signature"].as_str().unwrap(), "mainPair()" | "feeReceiver()" | "buyFeeReceiver()") {
                v & ((U256::one() << 160) - 1)
            } else {
                v
            };
            assert_eq!(c["execution"]["exit"]["data"], hex::encode(proof::vm::word(want)));
            assert_eq!(c["execution"]["committed_storage"], c["prestate"]);
        }
    }
}
#[test]
fn wkey2_buy_rollback_receiver_event_order_and_public_burn_from_are_source_specific() {
    let t = Target::Wkeydao2;
    let buy = record(t, "buy_all_fee_disabled", "transfer(address,uint256)");
    assert_eq!(buy["execution"]["exit"]["kind"], "revert");
    assert_eq!(buy["execution"]["writes"].as_array().unwrap().len(), 2);
    assert_eq!(buy["execution"]["logs"].as_array().unwrap().len(), 1);
    for (name, sig) in [
        ("receiver_10_change", "setFeeReceiver(address)"),
        ("receiver_11_change", "setBuyFeeReceiver(address)"),
    ] {
        let c = record(t, name, sig);
        assert_eq!(c["execution"]["writes"].as_array().unwrap().len(), 1);
        assert_eq!(c["execution"]["logs"].as_array().unwrap().len(), 1);
        assert!(c["execution"]["logs"][0]["step"].as_u64() < c["execution"]["writes"][0]["step"].as_u64());
    }
    let burn = record(t, "burn_from_balance_failure", "burnFrom(address,uint256)");
    assert_eq!(burn["execution"]["writes"].as_array().unwrap().len(), 1);
    assert_eq!(burn["execution"]["logs"].as_array().unwrap().len(), 1);
    assert_eq!(burn["execution"]["writes"][0]["key"], w(mapping(55.into(), mapping(44.into(), 1.into()))));
    let ordinary = record(t, "burn_ordinary", "burn(uint256)");
    assert_eq!(ordinary["execution"]["writes"].as_array().unwrap().len(), 2);
    assert_eq!(record(t, "mint_no_supply_cap", "mint(address,uint256)")["execution"]["exit"]["kind"], "return");
}
#[test]
fn trx_predicate_only_constructor_shape_and_inner_sender_controls_are_explicit() {
    let t = Target::Trx;
    for name in [
        "constructor_shaped_no_admin_grant",
        "self_role_zero_suffix",
        "self_role_wrong_suffix",
        "self_role_without_suffix",
        "self_role_truncated_suffix",
        "ordinary_role_forged_suffix",
    ] {
        assert_eq!(record(t, name, "grantRole(bytes32,address)")["execution"]["exit"]["kind"], "revert");
    }
    for (name, sender) in [("self_role_admin_suffix", 1), ("ordinary_role_ignores_suffix", 1)] {
        let c = record(t, name, "grantRole(bytes32,address)");
        assert_eq!(c["execution"]["logs"][0]["topics"][3], w(sender.into()));
    }
    let a = record(t, "self_approve_no_suffix", "approve(address,uint256)");
    assert_eq!(a["execution"]["logs"][0]["topics"][1], w(7.into()));
    let m = record(t, "self_predicate_mint_suffix", "mint(address,uint256)");
    assert_eq!(m["execution"]["logs"][0]["topics"][1], w(0.into()));
    assert_eq!(
        record(t, "constructor_shaped_mint_after_renounce", "mint(address,uint256)")["execution"]["exit"]["kind"],
        "revert"
    );
    assert_eq!(
        record(t, "constructor_shaped_repeat_renounce", "renounceRole(bytes32,address)")["execution"]["writes"],
        json!([])
    );
}
#[test]
fn unsupported_paths_stop_before_external_signature_or_catch_simulation() {
    for t in Target::ALL {
        for c in records(t).iter().filter(|c| c["scope"] == "unsupported_path_control") {
            assert_eq!(c["execution"]["exit"]["kind"], "harness_failure");
            assert_eq!(c["execution"]["committed_storage"], c["prestate"]);
            assert!(!c["execution"]["trace"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| matches!(s["opcode"].as_u64(), Some(0xf1 | 0xf4 | 0xfa))));
        }
    }
    let c = record(Target::Wkeydao2, "excluded_sell_receiver", "transfer(address,uint256)");
    assert_eq!(c["execution"]["writes"].as_array().unwrap().len(), 2);
    assert_eq!(c["execution"]["logs"].as_array().unwrap().len(), 2);
    let expiry = record(
        Target::Wkeydao2,
        "expired_permit",
        "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)",
    );
    assert_eq!(expiry["scope"], "synthetic_local_operation");
    assert_eq!(expiry["execution"]["exit"]["kind"], "revert");
    assert_eq!(expiry["timestamp"], w(2.into()));
}
#[test]
fn corrupted_store_missing_log_and_unexpected_invalid_are_saved_without_panicking() {
    for t in Target::ALL {
        let first = record(t, "role_add_empty", "grantRole(bytes32,address)");
        let c = proof::verify_capture(t.captured(), t).unwrap();
        for (field, op) in [("writes", 0x50), ("logs", 0x50), ("writes", 0xfe)] {
            let mut code = proof::runtime(&c).unwrap();
            let pc = first["execution"][field][0]["pc"].as_u64().unwrap() as usize;
            code[pc] = op;
            let mut p = Proof::new(&code, t);
            let mut attempts = vec![];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                p.matrix(&mut |_, v| {
                    attempts.push(v.clone());
                    Ok(())
                })
            }));
            assert!(result.is_ok());
            assert!(result.unwrap().is_err());
            assert!(!attempts.is_empty());
            assert_eq!(attempts[0]["name"], "role_add_empty");
        }
    }
}
