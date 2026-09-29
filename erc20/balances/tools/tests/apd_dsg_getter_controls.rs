#![cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use erc20_balances_tools::apd_dsg_controls::{
    self as p,
    cases::{self, Proof},
    projector, vm, Target,
};
use erc20_balances_tools::ptoken_proof::cases::get;
use primitive_types::U256;
use serde_json::{json, Value};
use std::sync::OnceLock;
use substreams_ethereum::pb::eth::v2 as eth;
struct Run {
    target: Target,
    records: Vec<Value>,
    operations: Vec<cases::Measured>,
    projections: Vec<Value>,
}
fn runs() -> &'static Vec<Run> {
    static RUNS: OnceLock<Vec<Run>> = OnceLock::new();
    RUNS.get_or_init(|| {
        [Target::Apd, Target::Dsg]
            .into_iter()
            .map(|t| {
                let (_, code) = p::bound(t).unwrap();
                let mut proof = Proof::new(t, &code);
                let mut save = |_: &Value| Ok(());
                proof.getters(&mut save).unwrap();
                if t == Target::Apd {
                    proof.apd_operations(&mut save).unwrap();
                } else {
                    proof.dsg_operations(&mut save).unwrap();
                    proof.dsg_saved(&mut save).unwrap();
                }
                let projections = proof.projections(&mut save).unwrap();
                Run {
                    target: t,
                    records: proof.records,
                    operations: proof.operations,
                    projections,
                }
            })
            .collect()
    })
}
#[test]
fn complete_original_captures_compiler_maps_transforms_and_both_boundaries_are_pinned() {
    for t in [Target::Apd, Target::Dsg] {
        let (c, r) = p::bound(t).unwrap();
        assert_eq!(p::kh(&r), t.runtime_hash());
        assert_eq!(
            c["runtimeBytecode"]["transformations"].as_array().unwrap().len(),
            if t == Target::Apd { 8 } else { 1 }
        );
        assert!(c["stdJsonOutput"]["contracts"][format!("{}.sol", t.name())][t.name()]["evm"]["deployedBytecode"]["generatedSources"].is_null());
    }
}
#[test]
fn binding_rejects_changed_source_address_chain_settings_runtime_and_extra_transform() {
    for t in [Target::Apd, Target::Dsg] {
        let c = p::verify_capture(t, t.captured()).unwrap();
        let mut variants = vec![];
        let mut v = c.clone();
        v["chainId"] = json!("1");
        variants.push(v);
        let mut v = c.clone();
        v["address"] = json!(Target::Apd.address());
        if t == Target::Apd {
            v["address"] = json!(Target::Dsg.address());
        }
        variants.push(v);
        let mut v = c.clone();
        v["sources"][format!("{}.sol", t.name())]["content"] = json!("");
        variants.push(v);
        let mut v = c.clone();
        v["stdJsonInput"]["settings"]["optimizer"]["runs"] = json!(201);
        variants.push(v);
        let mut v = c.clone();
        v["runtimeBytecode"]["onchainBytecode"] = json!("0x00");
        variants.push(v);
        let mut v = c.clone();
        v["runtimeBytecode"]["transformations"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"replace","offset":0,"reason":"cborAuxdata","id":"2"}));
        variants.push(v);
        for v in variants {
            assert!(p::verify_components(t, &v).is_err());
            assert!(p::verify_capture(t, &serde_json::to_vec(&v).unwrap()).is_err());
        }
    }
}
#[test]
fn immutable_width_offset_value_creation_append_and_cbor_mutations_are_refused() {
    let c = p::verify_capture(Target::Apd, Target::Apd.captured()).unwrap();
    for pointer in [
        "/runtimeBytecode/immutableReferences/831/0/start",
        "/runtimeBytecode/immutableReferences/831/0/length",
        "/runtimeBytecode/transformationValues/immutables/831",
        "/creationBytecode/transformationValues/constructorArguments",
        "/runtimeBytecode/transformationValues/cborAuxdata/1",
    ] {
        let mut v = c.clone();
        *v.pointer_mut(pointer).unwrap() = json!(0);
        assert!(p::verify_components(Target::Apd, &v).is_err());
    }
    let mut raw = Target::Dsg.captured().to_vec();
    raw.push(b' ');
    assert!(p::verify_capture(Target::Dsg, &raw).is_err());
}
#[test]
fn source_attribution_resolves_saved_solidity_and_explicitly_labels_missing_ids() {
    let c = p::verify_capture(Target::Apd, Target::Apd.captured()).unwrap();
    let map = p::source_map::decode(&[0x55], "0:10:99").unwrap();
    let missing = p::annotate(&c, 0, &map).unwrap();
    assert_eq!(missing["file_id"], 99);
    assert!(missing["path"].is_null());
    assert!(missing["source"].as_str().unwrap().contains("without captured body"));
    let map = p::source_map::decode(&[0x55], "0:10:0").unwrap();
    assert_eq!(p::annotate(&c, 0, &map).unwrap()["source_sha256"], Target::Apd.source_sha());
}
#[test]
fn selected_balance_word_zero_to_max_is_independent_of_holder_caller_and_all_metadata() {
    for r in runs() {
        let words = r.records.iter().filter(|v| v["name"] == "balance word domain").collect::<Vec<_>>();
        assert_eq!(words.len(), 32);
        let mut outputs = std::collections::BTreeSet::new();
        for v in words {
            assert_eq!(v["execution"]["exit"]["kind"], "return");
            let reads = v["execution"]["reads"].as_array().unwrap();
            assert_eq!(reads.len(), 1);
            outputs.insert(v["execution"]["exit"]["data"].clone().to_string());
        }
        assert_eq!(outputs.len(), 4);
        assert!(r.records.iter().any(|v| v["name"] == "balance independent of combined metadata"));
    }
}
#[test]
fn every_admitted_metadata_group_has_real_getter_read_witness_and_projector_control() {
    for r in runs() {
        for name in ["allowance", "nonce", "supply", "pair", "buy", "sell"] {
            let label = format!("{name} perturbation");
            assert!(r
                .records
                .iter()
                .any(|v| v["name"] == label && !v["execution"]["reads"].as_array().unwrap().is_empty()));
            assert!(r.projections.iter().any(|v| v["name"] == name && v["writes"] == 1));
        }
        if r.target == Target::Dsg {
            assert!(r.records.iter().any(|v| v["name"] == "burnt perturbation"));
        }
        assert!(r.projections.iter().all(|v| v["metadata_only"] == true));
    }
}
#[test]
fn apd_full_role_zero_member_noops_unauthorized_and_confirmation_are_source_checked() {
    let r = runs().iter().find(|r| r.target == Target::Apd).unwrap();
    assert_eq!(r.operations.iter().filter(|m| m.name == "APD role edge").count(), 16);
    assert_eq!(r.operations.iter().filter(|m| m.name == "APD unauthorized role").count(), 16);
    assert_eq!(r.operations.iter().filter(|m| m.name == "APD renounce").count(), 4);
    assert_eq!(r.operations.iter().filter(|m| m.name == "APD wrong confirmation").count(), 4);
    let grants = r
        .operations
        .iter()
        .filter(|m| m.name == "APD role edge" && m.execution.writes.is_empty())
        .count();
    assert_eq!(grants, 8);
    assert!(r.operations.iter().filter(|m| m.name == "APD role edge").any(|m| m
        .execution
        .writes
        .iter()
        .any(|w| w.key == cases::member(Target::Apd, cases::full_role(), 0.into()))));
}
#[test]
fn eleven_historical_dsg_operations_reexecute_exact_stores_and_preserve_balance_sentinels() {
    let r = runs().iter().find(|r| r.target == Target::Dsg).unwrap();
    let saved = p::saved_dsg().unwrap();
    for c in saved["cases"].as_array().unwrap() {
        let m = r.operations.iter().find(|m| m.name == c["name"]).unwrap();
        assert_eq!(m.execution.writes.len(), c["ordered_sstores"].as_array().unwrap().len());
        for h in cases::holders(Target::Dsg) {
            assert_eq!(get(&m.before, cases::balance(h)), get(&m.execution.committed, cases::balance(h)));
        }
    }
    assert_eq!(saved["cases"].as_array().unwrap().len(), 11);
}
#[test]
fn approvals_setter_limits_and_exact_reverts_have_getter_postconditions() {
    for r in runs() {
        assert!(r.records.iter().any(|v| v["signature"] == "allowance(address,address)"
            && v["name"]
                .as_str()
                .unwrap()
                .contains(if r.target == Target::Apd { "postcondition" } else { "approved" })));
        assert!(r.operations.iter().any(|m| m.name.contains("setter") || m.name.contains("ratio")));
        for m in &r.operations {
            if matches!(m.execution.exit, vm::Exit::Revert(_)) {
                assert_eq!(m.execution.committed, m.before);
                assert!(m.execution.committed_logs.is_empty());
            }
        }
    }
    let dsg = runs().iter().find(|r| r.target == Target::Dsg).unwrap();
    let apd = runs().iter().find(|r| r.target == Target::Apd).unwrap();
    for name in ["renounce postcondition", "APD tax postcondition", "APD pair postcondition"] {
        assert!(apd.records.iter().any(|r| r["name"] == name));
    }
    for name in ["DSG pair postcondition", "saved DSG role count", "saved DSG role member"] {
        assert!(dsg.records.iter().any(|r| r["name"] == name));
    }
    assert_eq!(dsg.operations.iter().filter(|m| m.name == "DSG approve/reset").count(), 3);
    assert_eq!(dsg.operations.iter().filter(|m| m.name == "DSG ratio boundary").count(), 9);
}
#[test]
fn mixed_projector_balance_agrees_with_recorded_runtime_getter_for_every_success() {
    for r in runs() {
        let successes = r.operations.iter().filter(|m| matches!(m.execution.exit, vm::Exit::Return(_))).count();
        assert_eq!(r.projections.iter().filter(|v| v["mixed_balance_getter_combinations"] == 2).count(), successes);
        assert!(r
            .projections
            .iter()
            .filter(|v| v["kind"].is_string())
            .all(|v| v["mixed_balance_getter_combinations"] == 1));
    }
}
fn empty(t: Target) -> eth::Call {
    eth::Call {
        address: hex::decode(&t.address()[2..]).unwrap(),
        begin_ordinal: 1,
        end_ordinal: 99999,
        ..Default::default()
    }
}
#[test]
fn excluded_token_fields_and_restored_changes_remain_refusals_without_balance_output() {
    for t in [Target::Apd, Target::Dsg] {
        let mut slots = if t == Target::Apd {
            vec![3.into(), 4.into(), 8.into(), 9.into()]
        } else {
            vec![3.into(), 4.into(), 5.into(), 10.into()]
        };
        slots.push(cases::head(t, cases::full_role()) + U256::from(if t == Target::Apd { 1 } else { 2 }));
        for key in slots {
            let mut c = empty(t);
            let data = [vm::word(cases::full_role()), vm::word(t.role_root())].concat();
            c.keccak_preimages.insert(hex::encode(vm::word(vm::hash(&data))), hex::encode(data));
            c.storage_changes = vec![projector::row(t, key, 0.into(), 1.into(), 10), projector::row(t, key, 1.into(), 0.into(), 20)];
            let b = projector::synthetic(t, c).unwrap();
            assert!(projector::project(t, &b).is_err(), "{} excluded {}", t.label(), key);
        }
    }
}
#[test]
fn captured_context_runtime_excursion_refuses_and_reverted_excursion_is_ignored() {
    for t in [Target::Apd, Target::Dsg] {
        let mut b = projector::captured(t).unwrap();
        for c in b.system_calls.iter_mut().chain(b.transaction_traces.iter_mut().flat_map(|t| &mut t.calls)) {
            c.storage_changes.clear();
            c.code_changes.clear();
        }
        let tx = b
            .transaction_traces
            .iter_mut()
            .find(|tx| tx.status() == eth::TransactionTraceStatus::Succeeded && tx.calls.iter().any(|c| !c.state_reverted))
            .unwrap();
        let c = tx.calls.iter_mut().find(|c| !c.state_reverted).unwrap();
        let account = hex::decode(&t.address()[2..]).unwrap();
        let bound = hex::decode(&t.runtime_hash()[2..]).unwrap();
        let bad = vec![0x81; 32];
        c.code_changes = vec![
            eth::CodeChange {
                address: account.clone(),
                old_hash: bound.clone(),
                new_hash: bad.clone(),
                ordinal: c.begin_ordinal + 1,
                ..Default::default()
            },
            eth::CodeChange {
                address: account,
                old_hash: bad,
                new_hash: bound,
                ordinal: c.begin_ordinal + 2,
                ..Default::default()
            },
        ];
        assert!(projector::project(t, &b).is_err());
        for c in b
            .transaction_traces
            .iter_mut()
            .flat_map(|t| &mut t.calls)
            .filter(|c| !c.code_changes.is_empty())
        {
            c.state_reverted = true;
        }
        assert!(projector::project(t, &b).unwrap().balances.is_empty());
    }
}
#[test]
fn missing_mapping_preimage_and_partial_dsg_set_write_do_not_gain_permission() {
    for t in [Target::Apd, Target::Dsg] {
        let mut c = empty(t);
        c.storage_changes
            .push(projector::row(t, cases::allowance(44.into(), 55.into()), 0.into(), 1.into(), 10));
        assert!(projector::project(t, &projector::synthetic(t, c).unwrap()).is_err());
    }
    let dsg = runs().iter().find(|r| r.target == Target::Dsg).unwrap();
    let m = dsg.operations.iter().find(|m| m.name == "add_empty").unwrap();
    let mut b = projector::measured(m, false).unwrap();
    b.transaction_traces[0].calls[0].storage_changes.pop();
    assert!(projector::project(Target::Dsg, &b).is_err());
}
#[test]
fn wrong_getter_bytecode_and_corrupted_witnesses_fail_without_hiding_attempt() {
    let mut saved = 0;
    let mut proof = Proof::new(Target::Apd, &[0]);
    assert!(proof
        .getters(&mut |_| {
            saved += 1;
            Ok(())
        })
        .is_err());
    assert_eq!(saved, 1);
    let m = &runs()[0].operations[0];
    let mut e = m.execution.clone();
    e.writes[0].new ^= U256::one();
    assert!(cases::validate_witnesses(&m.before, &e).is_err());
    let mut e = m.execution.clone();
    e.logs.clear();
    assert!(cases::check_operation(
        &m.before,
        &e,
        &m.execution.exit,
        &m.execution.writes.iter().map(|w| (w.key, w.new)).collect::<Vec<_>>(),
        &m.execution.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>()
    )
    .is_err());
    let mut e = m.execution.clone();
    e.logs[0].step = 0;
    assert!(cases::check_operation(
        &m.before,
        &e,
        &m.execution.exit,
        &m.execution.writes.iter().map(|w| (w.key, w.new)).collect::<Vec<_>>(),
        &m.execution.logs.iter().map(|l| (l.topics.clone(), l.data.clone())).collect::<Vec<_>>()
    )
    .is_err());
}
#[test]
fn raw_nonce_or_ratio_max_is_not_misrepresented_as_permit_or_setter_success() {
    for r in runs() {
        assert!(r
            .records
            .iter()
            .filter(|v| v["name"] == "nonce perturbation")
            .any(|v| v["execution"]["exit"]["data"] == hex::encode(vm::word(U256::max_value()))));
        assert!(r.records.iter().all(|v| !v["signature"].as_str().unwrap().starts_with("permit")));
    }
}
#[test]
fn raw_callback_failure_propagates_before_getter_assertions() -> Result<()> {
    let (_, code) = p::bound(Target::Apd)?;
    let mut proof = Proof::new(Target::Apd, &code);
    let e = proof.getters(&mut |_| anyhow::bail!("deliberate artifact write failure"));
    assert!(format!("{:#}", e.unwrap_err()).contains("artifact write failure"));
    assert!(proof.records.is_empty());
    Ok(())
}
