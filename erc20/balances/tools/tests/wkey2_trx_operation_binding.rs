#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::wkey2_trx_proof::{self as p, Target};
use primitive_types::U256;
use serde_json::{json, Value};
use std::{fs, path::Path};
fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/wkey2-trx-operation-proof")
}
#[test]
fn wkey2_trx_exact_whole_capture_and_code_bindings() {
    for t in Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        p::verify_layout(&c["storageLayout"], t).unwrap();
        assert_eq!(p::runtime(&c).unwrap().len(), t.runtime_len());
        for (field, creation) in [("runtimeBytecode", false), ("creationBytecode", true)] {
            let code = p::bytes(&c[field]["recompiledBytecode"]).unwrap();
            let patched = p::patched(&code, t, creation).unwrap();
            let off = t.cbor_offset(creation);
            assert_eq!(&code[..off], &patched[..off]);
            assert_eq!(&code[off + 53..], &patched[off + 53..]);
            assert_ne!(&code[off..off + 53], &patched[off..off + 53]);
            let full = if creation { [patched, p::arguments(t)].concat() } else { patched };
            assert_eq!(full, p::bytes(&c[field]["onchainBytecode"]).unwrap());
        }
        assert_eq!(c["match"], "match");
        assert_eq!(c["runtimeMatch"], "match");
        assert_eq!(c["creationMatch"], "match");
        assert_eq!(t.creation_len() - t.cbor_offset(true) - 53, if t == Target::Wkeydao2 { 32 } else { 79 });
    }
}
#[test]
fn wkey2_trx_complete_capture_tampering_is_refused_before_attribution() {
    for t in Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        for path in [
            "address",
            "chainId",
            "sources",
            "metadata",
            "stdJsonInput",
            "stdJsonOutput",
            "sourceIds",
            "runtimeBytecode",
            "creationBytecode",
            "storageLayout",
            "deployment",
            "match",
        ] {
            let mut bad = c.clone();
            bad[path] = Value::Null;
            assert!(p::verify_components(&bad, t).is_err(), "{path}");
            assert!(p::verify_capture(&serde_json::to_vec(&bad).unwrap(), t).is_err());
        }
        let other = if t == Target::Wkeydao2 { Target::Trx } else { Target::Wkeydao2 };
        assert!(p::verify_capture(t.captured(), other).is_err());
    }
}
#[test]
fn wkey2_trx_cbor_is_exact_bounded_substitution_not_suffix_stripping() {
    for t in Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        for (field, creation) in [("runtimeBytecode", false), ("creationBytecode", true)] {
            let code = p::bytes(&c[field]["recompiledBytecode"]).unwrap();
            for delta in [0, 10, 42, 52] {
                let mut bad = code.clone();
                bad[t.cbor_offset(creation) + delta] ^= 1;
                assert!(p::patched(&bad, t, creation).is_err());
            }
            assert!(p::patched(&code[..code.len() - 1], t, creation).is_err());
            // Unchanged non-CBOR bytes are not independently trusted by the patch
            // helper: complete fresh output/capture equality is a separate gate.
            let mut bad = code.clone();
            bad[0] ^= 1;
            assert_ne!(p::patched(&bad, t, creation).unwrap(), p::patched(&code, t, creation).unwrap());
        }
    }
}
#[test]
fn wkey2_trx_independent_constructor_abis_are_exact() {
    let a = p::arguments(Target::Wkeydao2);
    assert_eq!(a.len(), 96);
    assert_eq!(U256::from_big_endian(&a[..32]), p::address("0xea52fe6730078b5a55c26971ec3351eba873aa91"));
    assert_eq!(U256::from_big_endian(&a[32..64]), p::address("0x14734534efc59d3dcdbecccfe79c74fda0e124a8"));
    assert_eq!(U256::from_big_endian(&a[64..]), 100000.into());
    let a = p::arguments(Target::Trx);
    assert_eq!(a.len(), 192);
    assert_eq!(U256::from_big_endian(&a[..32]), 64.into());
    assert_eq!(U256::from_big_endian(&a[32..64]), 128.into());
    for (off, s) in [(64, "TRON"), (128, "TRX")] {
        assert_eq!(U256::from_big_endian(&a[off..off + 32]), s.len().into());
        assert_eq!(&a[off + 32..off + 32 + s.len()], s.as_bytes());
        assert!(a[off + 32 + s.len()..off + 64].iter().all(|b| *b == 0));
    }
}
#[test]
fn wkey2_trx_fresh_full_outputs_and_auxiliary_artifacts_are_bound() {
    let root = fixture();
    let manifest = fs::read(root.join("solc-list.json")).unwrap();
    for t in Target::ALL {
        let dir = root.join(t.label());
        let c = p::verify_capture(t.captured(), t).unwrap();
        p::verify_manifest(&manifest, t).unwrap();
        let raw = fs::read(dir.join("compiler-output.json")).unwrap();
        p::verify_compiled(&c, &raw, t).unwrap();
        p::verify_auxiliary(&dir, &c, t).unwrap();
        let mut out: Value = serde_json::from_slice(&raw).unwrap();
        out["contracts"][t.source()][t.name()]["evm"]["deployedBytecode"]["object"] = json!("00");
        assert!(p::verify_compiled(&c, &serde_json::to_vec(&out).unwrap(), t).is_err());
        assert!(p::check_compiled(&c, &serde_json::to_vec(&out).unwrap(), t).is_err());
        let mut input = p::input(&c).unwrap();
        let selected = input["settings"].as_object_mut().unwrap().remove("outputSelection");
        assert!(selected.is_some());
        assert_eq!(input, c["stdJsonInput"]);
    }
}
#[test]
fn wkey2_trx_primary_files_and_normalized_declarations_remain_distinct() {
    for t in Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let raw = fs::read(fixture().join(t.label()).join("primary-sources.json")).unwrap();
        let primary: Value = serde_json::from_slice(&raw).unwrap();
        p::verify_primary(&c, &primary, t).unwrap();
        let exact = primary["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["classification"] == "exact_full_file")
            .count();
        assert_eq!(exact, if t == Target::Wkeydao2 { 4 } else { 0 });
        let mut bad = primary.clone();
        bad["sources"] = json!([]);
        assert!(p::verify_primary(&c, &bad, t).is_err());
        if t == Target::Trx {
            let raw = c["sources"][t.source()]["content"].as_str().unwrap();
            assert!(raw.contains("\r\n"));
            assert_eq!(primary["declarations"].as_array().unwrap().len(), 2);
            let mut bad = primary.clone();
            bad["declarations"][0]["normalization"] = json!("whole-file exact");
            assert!(p::verify_primary(&c, &bad, t).is_err());
            let mut bad = primary.clone();
            bad["declarations"][0]["primary_content"] = json!("library Something {}");
            assert!(p::verify_primary(&c, &bad, t).is_err());
        }
    }
}
fn trace(pc: usize, op: u8, src: U256, len: U256) -> Value {
    json!({"trace":[{"pc":pc,"opcode":op,"stack_top":[format!("0x{}",hex::encode(p::vm::word(0.into()))),format!("0x{}",hex::encode(p::vm::word(src))),format!("0x{}",hex::encode(p::vm::word(len)))]}]})
}
#[test]
fn wkey2_trx_trace_guard_rejects_cbor_instructions_immediates_and_copy_overlap() {
    for t in Target::ALL {
        for creation in [false, true] {
            let start = t.cbor_offset(creation);
            let end = start + 53;
            assert!(p::verify_trace(&trace(start - 1, 0x00, 0.into(), 0.into()), t, creation).is_ok());
            assert!(p::verify_trace(&trace(start, 0x00, 0.into(), 0.into()), t, creation).is_err());
            assert!(p::verify_trace(&trace(start - 1, 0x60, 0.into(), 0.into()), t, creation).is_err());
            assert!(p::verify_trace(&trace(start - 32, 0x7f, 0.into(), 0.into()), t, creation).is_err());
            for (src, len, ok) in [
                (start - 1, 1, true),
                (start - 1, 2, false),
                (start, 0, true),
                (start, 1, false),
                (end - 1, 1, false),
                (end, 1, true),
            ] {
                assert_eq!(p::verify_trace(&trace(0, 0x39, src.into(), len.into()), t, creation).is_ok(), ok);
            }
            assert!(p::verify_trace(&trace(0, 0x39, U256::MAX, 1.into()), t, creation).is_err());
            assert!(p::verify_trace(&trace(end, 0x00, 0.into(), 0.into()), t, creation).is_ok());
        }
    }
}
#[test]
fn trx_only_exact_generated_effect_pcs_keep_absent_source_text_explicit() {
    use std::collections::BTreeMap;
    let t = Target::Trx;
    let c = p::verify_capture(t.captured(), t).unwrap();
    let compiled = p::verify_compiled(&c, &fs::read(fixture().join("trx/compiler-output.json")).unwrap(), t).unwrap();
    for (creation, kind, read_pc, write_pc) in [(true, "bytecode", 1372, 1379), (false, "deployedBytecode", 6169, 6176)] {
        let v = &compiled["contracts"][t.source()][t.name()]["evm"][kind];
        let code = p::bytes(&v["object"]).unwrap();
        let map = p::source_map::decode(&code, v["sourceMap"].as_str().unwrap()).unwrap();
        let sources = p::mapped_sources(&c, &compiled, t, kind).unwrap();
        let generated: Vec<_> = map
            .iter()
            .filter(|(pc, s)| s.file == -1 && [0x54, 0x55, 0x20, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4].contains(&code[**pc]))
            .map(|(pc, _)| *pc)
            .collect();
        assert_eq!(generated, [read_pc, write_pc]);
        for (pc, field) in [(read_pc, "reads"), (write_pc, "writes")] {
            let annotation = p::annotate_effect(pc, field, t, creation, &code, &map, &sources).unwrap();
            assert_eq!(annotation["file_id"], -1);
            assert_eq!(annotation["solidity_source_attributed"], false);
            assert!(annotation.get("path").is_none());
            assert!(annotation.get("text").is_none());
            assert_eq!(annotation["compiler_output_sha256"], t.compiled_sha());
            assert!(p::annotate_effect(pc, "logs", t, creation, &code, &map, &sources).is_err());
            assert!(p::annotate_effect(pc, field, Target::Wkeydao2, creation, &code, &map, &sources).is_err());
            assert!(p::annotate_effect(pc, field, t, !creation, &code, &map, &sources).is_err());
            let mut changed = map.clone();
            changed.get_mut(&pc).unwrap().start += 1;
            assert!(p::annotate_effect(pc, field, t, creation, &code, &changed, &sources).is_err());
            let mut changed = code.clone();
            changed[pc] ^= 1;
            assert!(p::annotate_effect(pc, field, t, creation, &changed, &map, &sources).is_err());
            assert!(p::annotate_effect(pc, field, t, creation, &code, &BTreeMap::new(), &sources).is_err());
        }
    }
}
