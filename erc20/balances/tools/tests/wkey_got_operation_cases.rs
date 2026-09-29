#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::ptoken_proof::cases::{mapping, role, w};
use erc20_balances_tools::wkey_got_proof::{
    self as proof,
    cases::{admin, element, head, position, Proof},
    Target,
};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{path::Path, sync::OnceLock};

fn records() -> &'static [Value] {
    static RECORDS: OnceLock<Vec<Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let mut all = vec![];
        for t in Target::ALL {
            let capture = proof::verify_capture(t.captured(), t).unwrap();
            let output = std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/wkey-got-operation-proof")
                    .join(t.label())
                    .join("compiler-output.json"),
            )
            .unwrap();
            proof::verify_compiled(&capture, &output, t).unwrap();
            let runtime = proof::runtime(&capture).unwrap();
            let mut p = Proof::new(&runtime, t);
            let mut count = 0;
            let mut save = |n: usize, v: &Value| {
                assert_eq!(n, count + 1);
                assert_eq!(v["target"], t.label());
                assert!(v["execution"]["trace"].is_array());
                count = n;
                Ok(())
            };
            p.constructor(&capture, &mut save).unwrap();
            p.matrix(&mut save).unwrap();
            assert_eq!(p.calls, count);
            assert_eq!(p.calls, p.cases.len());
            assert!(p.calls > 400);
            all.extend(p.cases);
        }
        all
    })
}
fn record(t: Target, name: &str, signature: &str) -> &'static Value {
    records()
        .iter()
        .find(|r| r["target"] == t.label() && r["name"] == name && r["signature"] == signature)
        .unwrap_or_else(|| panic!("missing {} {name}: {signature}", t.label()))
}
fn n(v: &Value) -> U256 {
    U256::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
}
fn state(v: &Value) -> proof::vm::State {
    v.as_array().unwrap().iter().map(|x| (n(&x["key"]), n(&x["value"]))).collect()
}
fn kind(v: &Value) -> &str {
    v["execution"]["exit"]["kind"].as_str().unwrap()
}
fn arg_code(v: &Value) -> Vec<u8> {
    hex::decode(v["constructor_code"].as_str().unwrap()).unwrap()
}
#[test]
fn wkey_got_complete_matrix_has_exact_failure_classes_and_code_identities() {
    for r in records() {
        let t = if r["target"] == "wkeydao" { Target::Wkeydao } else { Target::Got };
        let k = kind(r);
        assert!(matches!(k, "return" | "revert" | "invalid" | "harness_failure"));
        assert_eq!(k == "invalid", r["scope"] == "source_invalid_control");
        assert_eq!(k == "harness_failure", r["scope"] == "unsupported_path_control");
        assert_eq!(r["address"], w(t.account()));
        if k != "return" {
            assert_eq!(r["execution"]["committed_storage"], r["prestate"]);
            assert_eq!(r["execution"]["committed_logs"], 0);
        }
        if r["code_kind"] == "bytecode" {
            let code = arg_code(r);
            assert_eq!(proof::sha(&code), r["code_sha256"]);
            assert_eq!(code.len(), t.creation_len() + 96);
            assert_eq!(r["self_code_size"], 0);
        } else {
            assert!(r["constructor_code"].is_null());
            let capture: Value = serde_json::from_slice(t.captured()).unwrap();
            assert_eq!(proof::sha(&proof::runtime(&capture).unwrap()), r["code_sha256"]);
            assert_eq!(r["self_code_size"], t.runtime_len());
        }
    }
}
#[test]
fn wkey_constructor_preserves_its_full_prefix_before_unsupported_chainid() {
    let r = record(Target::Wkeydao, "wkeydao_constructor", "constructor(address,address,uint256)");
    assert_eq!(kind(r), "harness_failure");
    let writes = r["execution"]["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 5);
    assert_eq!(
        writes.iter().map(|s| n(&s["key"])).collect::<Vec<_>>(),
        [13.into(), 3.into(), 4.into(), 5.into(), 6.into()]
    );
    assert_eq!(writes[0]["new"], w(60000.into()));
    assert_eq!(writes[4]["new"], w(U256::from(115_000_000u64) * U256::from(1_000_000_000u64)));
    for s in writes {
        assert_eq!(s["old"], w(0.into()));
    }
    assert_eq!(r["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0x46);
    assert_eq!(r["execution"]["logs"], json!([]));
    assert_eq!(r["execution"]["committed_storage"], json!([]));
}
#[test]
fn got_constructor_success_and_rejection_preserve_independent_arguments_and_prefixes() {
    let t = Target::Got;
    let capture: Value = serde_json::from_slice(t.captured()).unwrap();
    let prefix = proof::bytes(&capture["creationBytecode"]["recompiledBytecode"]).unwrap();
    let runtime = proof::runtime(&capture).unwrap();
    for r in records().iter().filter(|r| r["target"] == "got" && r["code_kind"] == "bytecode") {
        let code = arg_code(r);
        assert_eq!(&code[..prefix.len()], prefix);
        let args: Vec<_> = code[prefix.len()..].chunks_exact(32).map(U256::from_big_endian).collect();
        let caller = n(&r["caller"]);
        let writes = r["execution"]["writes"].as_array().unwrap();
        assert_eq!(
            writes.iter().take(5).map(|s| n(&s["key"])).collect::<Vec<_>>(),
            [12.into(), 3.into(), 4.into(), 5.into(), 6.into()]
        );
        assert_eq!(writes[0]["new"], w(3000.into()));
        // Base VaultOwned grants before any derived constructor require.
        assert_eq!(writes[5]["key"], w(head(t, 0.into())));
        assert_eq!(writes[6]["key"], w(element(t, 0.into(), 0.into())));
        assert_eq!(writes[7]["key"], w(position(t, 0.into(), caller)));
        assert_eq!(
            r["execution"]["logs"][0]["topics"],
            json!([w(role("RoleGranted(bytes32,address,address)")), w(0.into()), w(caller), w(caller)])
        );
        if kind(r) == "revert" {
            assert_eq!(writes.len(), 8);
            assert_eq!(r["execution"]["logs"].as_array().unwrap().len(), 1);
            assert_eq!(r["execution"]["committed_storage"], json!([]));
        } else {
            assert_eq!(kind(r), "return");
            assert_eq!(r["execution"]["exit"]["data"], hex::encode(&runtime));
            let post = state(&r["execution"]["committed_storage"]);
            assert_eq!(post[&10.into()], args[0]);
            assert_eq!(post[&11.into()], args[1]);
            assert_eq!(post[&13.into()], args[2]);
            assert_eq!(post[&head(t, 0.into())], U256::one());
            assert_eq!(post[&position(t, 0.into(), caller)], U256::one());
            let distinct = args[0] != caller;
            assert_eq!(post[&head(t, role("INTERN_SYSTEM"))], U256::from(if distinct { 2 } else { 1 }));
            assert_eq!(writes.len(), if distinct { 17 } else { 14 });
            assert_eq!(r["execution"]["logs"].as_array().unwrap().len(), if distinct { 3 } else { 2 });
            assert!(!post.contains_key(&2.into()));
        }
    }
    assert_eq!(records().iter().filter(|r| r["target"] == "got" && r["code_kind"] == "bytecode").count(), 7);
    let original = record(t, "got_constructor", "constructor(address,address,uint256)");
    assert_eq!(arg_code(original), proof::bytes(&capture["creationBytecode"]["onchainBytecode"]).unwrap());
}
#[test]
fn wkey_got_roles_keep_legacy_self_swaps_zero_equalities_and_noops() {
    for t in Target::ALL {
        let r = role("TEST_ROLE");
        for (name, member) in [
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
            assert_eq!(writes[1]["key"], w(position(t, r, member.into())));
            assert_eq!(writes[4]["key"], w(position(t, r, member.into())));
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
            assert!(record(t, name, sig)["execution"]["writes"]
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
            let c = record(t, name, sig);
            assert_eq!(c["execution"]["writes"], json!([]));
            assert_eq!(c["execution"]["logs"], json!([]));
            assert!(!c["execution"]["reads"].as_array().unwrap().is_empty());
        }
        let pre = state(&record(t, "role_add_empty", "grantRole(bytes32,address)")["prestate"]);
        assert_eq!(pre[&6.into()], U256::from(1000));
        assert_eq!(pre[&mapping(44.into(), 7.into())], U256::from(17));
        if t == Target::Wkeydao {
            assert_eq!(pre[&8.into()], role("DOMAIN_SENTINEL"));
        }
    }
}
#[test]
fn wkey_got_malformed_set_invalids_are_strictly_named_and_rollback() {
    for t in Target::ALL {
        for name in ["empty_nonzero_position", "out_of_bounds_position", "max_position"] {
            let c = record(t, name, "revokeRole(bytes32,address)");
            assert_eq!(kind(c), "invalid");
            assert_eq!(c["scope"], "source_invalid_control");
            assert_eq!(c["execution"]["writes"], json!([]));
            assert_eq!(c["execution"]["logs"], json!([]));
            assert_eq!(c["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], 0xfe);
        }
        let c = record(t, "malformed_max_length_push", "grantRole(bytes32,address)");
        let r = role("MALFORMED_ROLE");
        assert_eq!(c["execution"]["writes"][0]["key"], w(head(t, r)));
        assert_eq!(c["execution"]["writes"][0]["old"], w(U256::MAX));
        assert_eq!(c["execution"]["writes"][0]["new"], w(0.into()));
        assert_eq!(c["execution"]["writes"][1]["key"], w(element(t, r, U256::MAX)));
        assert_eq!(c["execution"]["writes"][2]["new"], w(0.into()));
        let bounds: Vec<_> = records()
            .iter()
            .filter(|r| r["target"] == t.label() && r["signature"] == "getRoleMember(bytes32,uint256)" && kind(r) == "revert")
            .collect();
        assert!(!bounds.is_empty());
        for c in bounds {
            assert!(c["execution"]["exit"]["data"].as_str().unwrap().starts_with("08c379a0"));
        }
    }
}
#[test]
fn wkey_got_admin_authorization_and_last_admin_loss_preserve_all_state() {
    for t in Target::ALL {
        let c = record(t, "custom_admin_authorized", "grantRole(bytes32,address)");
        assert_eq!(state(&c["execution"]["committed_storage"])[&admin(t, role("TEST_ROLE"))], role("CUSTOM_ADMIN"));
        let c = record(t, "max_admin_authorized", "grantRole(bytes32,address)");
        assert_eq!(state(&c["execution"]["committed_storage"])[&admin(t, role("TEST_ROLE"))], U256::MAX);
        for (name, sig) in [
            ("wrong_grant_admin", "grantRole(bytes32,address)"),
            ("wrong_revoke_admin", "revokeRole(bytes32,address)"),
            ("wrong_renounce_account", "renounceRole(bytes32,address)"),
        ] {
            let c = record(t, name, sig);
            assert_eq!(kind(c), "revert");
            assert_eq!(c["execution"]["writes"], json!([]));
            assert_eq!(c["execution"]["logs"], json!([]));
        }
        let lost = record(t, "last_admin_renounce", "renounceRole(bytes32,address)");
        let denied = record(t, "grant_after_last_admin_renounce", "grantRole(bytes32,address)");
        assert_eq!(lost["execution"]["committed_storage"], denied["prestate"]);
        assert_eq!(kind(denied), "revert");
        assert_eq!(state(&denied["prestate"])[&head(t, 0.into())], U256::zero());
        assert_eq!(
            record(t, "no_public_admin_setter", "setRoleAdmin(bytes32,bytes32)")["execution"]["exit"]["data"],
            ""
        );
        assert_eq!(
            record(t, "dirty_address_abi", "grantRole(bytes32,address)")["execution"]["logs"][0]["topics"][2],
            w(3.into())
        );
    }
}
#[test]
fn wkey_got_raw_getters_use_target_specific_slots_and_mask_only_addresses() {
    for c in records().iter().filter(|r| r["name"].as_str().unwrap().starts_with("raw_getter_")) {
        let reads = c["execution"]["reads"].as_array().unwrap();
        assert_eq!(reads.len(), 1);
        let value = n(&reads[0]["value"]);
        let sig = c["signature"].as_str().unwrap();
        let expected = if ["mainPair()", "feeReceiver()", "buyFeeReceiver()"].contains(&sig) {
            value & ((U256::one() << 160) - 1)
        } else {
            value
        };
        assert_eq!(c["execution"]["exit"]["data"], hex::encode(proof::vm::word(expected)));
        if sig == "nonces(address)" {
            assert_eq!(reads[0]["key"], w(mapping(44.into(), 7.into())));
        }
        if sig == "MaxSupply()" {
            assert_eq!(reads[0]["key"], w(6.into()));
        }
        if sig == "DOMAIN_SEPARATOR()" {
            assert_eq!(c["target"], "wkeydao");
            assert_eq!(reads[0]["key"], w(8.into()));
        }
    }
    assert_eq!(kind(record(Target::Got, "got_has_no_domain_getter", "DOMAIN_SEPARATOR()")), "revert");
}
#[test]
fn wkey_got_mint_burn_cap_and_approval_prefixes_match_source_order() {
    for t in Target::ALL {
        let mint = record(t, "mint_ordinary", "mint(address,uint256)");
        assert_eq!(mint["execution"]["logs"][0]["topics"][1], w(t.account()));
        let capped = record(t, "mint_cap_exact", "mint(address,uint256)");
        let post = state(&capped["execution"]["committed_storage"]);
        assert_eq!(post[&2.into()], post[&6.into()]);
        assert_eq!(kind(record(t, "mint_cap_excess", "mint(address,uint256)")), "revert");
        let late = record(t, "mint_balance_overflow", "mint(address,uint256)");
        assert_eq!(late["execution"]["writes"].as_array().unwrap().len(), 1);
        assert_eq!(late["execution"]["writes"][0]["key"], w(2.into()));
        let burn = record(t, "burn_ordinary", "burn(uint256)");
        assert_eq!(
            burn["execution"]["writes"].as_array().unwrap().iter().map(|s| n(&s["key"])).collect::<Vec<_>>(),
            [mapping(44.into(), 0.into()), 2.into(), 6.into()]
        );
        assert_eq!(burn["execution"]["writes"][2]["new"], w(993.into()));
        for (name, count) in [("burn_supply_failure", 1), ("burn_cap_failure", 2)] {
            let c = record(t, name, "burn(uint256)");
            assert_eq!(kind(c), "revert");
            assert_eq!(c["execution"]["writes"].as_array().unwrap().len(), count);
        }
        let c = record(t, "burn_from_cap_failure", "burnFrom(address,uint256)");
        assert_eq!(kind(c), "revert");
        assert_eq!(c["execution"]["writes"].as_array().unwrap().len(), 3);
        assert_eq!(c["execution"]["logs"].as_array().unwrap().len(), 1);
        assert_eq!(c["execution"]["logs"][0]["topics"][0], w(role("Approval(address,address,uint256)")));
        let public = record(t, "burn_from_ordinary", "_burnFrom(address,uint256)");
        assert_eq!(kind(public), if t == Target::Wkeydao { "return" } else { "revert" });
        if t == Target::Got {
            assert_eq!(public["execution"]["exit"]["data"], "");
            assert_eq!(public["execution"]["writes"], json!([]));
        }
    }
}
#[test]
fn wkey_got_fee_arithmetic_aliases_and_excluded_calls_are_separate() {
    for t in Target::ALL {
        let wrapped = record(t, "buy_fee_unchecked_receiver_wrap", "transfer(address,uint256)");
        assert_eq!(kind(wrapped), "return");
        assert_eq!(wrapped["execution"]["writes"][1]["old"], w(U256::MAX));
        assert_eq!(wrapped["execution"]["writes"][1]["new"], w(1.into()));
        let alias = record(t, "buy_fee_recipient_alias", "transfer(address,uint256)");
        assert_eq!(alias["execution"]["writes"][1]["key"], alias["execution"]["writes"][2]["key"]);
        assert_eq!(alias["execution"]["writes"][2]["old"], w(2.into()));
        assert_eq!(alias["execution"]["writes"][2]["new"], w(100.into()));
        let failure = record(t, "transfer_from_late_allowance_failure", "transferFrom(address,address,uint256)");
        assert_eq!(kind(failure), "revert");
        assert_eq!(failure["execution"]["writes"].as_array().unwrap().len(), 2);
        assert_eq!(failure["execution"]["logs"].as_array().unwrap().len(), 1);
        for (name, sig, opcode) in [
            ("excluded_fee_receiver", "transfer(address,uint256)", 0x3b),
            ("excluded_permit", "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)", 0x42),
        ] {
            let c = record(t, name, sig);
            assert_eq!(kind(c), "harness_failure");
            assert_eq!(c["execution"]["trace"].as_array().unwrap().last().unwrap()["opcode"], opcode);
        }
        let fee = record(t, "excluded_fee_receiver", "transfer(address,uint256)");
        assert_eq!(fee["execution"]["writes"].as_array().unwrap().len(), 2);
        assert_eq!(fee["execution"]["logs"].as_array().unwrap().len(), 2);
        assert_eq!(fee["execution"]["logs"][1]["topics"][0], w(role("FeeTaken(address,address,uint256,uint256)")));
        assert_eq!(fee["execution"]["exit"]["error"], "EXTCODESIZE unknown external account");
    }
}
#[test]
fn wkey_got_missing_store_log_or_unexpected_invalid_is_a_saved_reportable_failure() {
    for t in Target::ALL {
        let capture = proof::verify_capture(t.captured(), t).unwrap();
        let first = record(t, "role_add_empty", "grantRole(bytes32,address)");
        for (field, replacement) in [("writes", 0x50), ("logs", 0x50), ("writes", 0xfe)] {
            let mut code = proof::runtime(&capture).unwrap();
            let pc = first["execution"][field][0]["pc"].as_u64().unwrap() as usize;
            code[pc] = replacement;
            let mut p = Proof::new(&code, t);
            let mut attempts = vec![];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                p.matrix(&mut |_, v| {
                    attempts.push(v.clone());
                    Ok(())
                })
            }));
            assert!(result.is_ok(), "negative controls return errors, never panic");
            assert!(result.unwrap().is_err());
            assert!(!attempts.is_empty());
            assert_eq!(attempts[0]["name"], "role_add_empty");
            assert!(attempts[0]["execution"]["trace"].is_array());
        }
    }
}
