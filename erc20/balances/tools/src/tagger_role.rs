//! Exact saved-source/bytecode binding for one NOT-QUALIFIED Tagger candidate.
//! Offline only. This does not relax the other candidates' transformation rules.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const FIXTURE: &str = "tests/fixtures/tagger-role-candidate";
pub const CONTRACT: &str = "0x208bf3e7da9639f1eaefa2de78c23396b0682025";
pub const CAPTURE: &str = "5fac5c544be56adfb7779efabf5a23d435fcb7de04c37af0c6de2e1700fb5663";
pub const SOURCE_SHA: &str = "a986e195f98b68a206a95aa0b591c5b527a0186fb62d4a5238430fc30f20e0eb";
pub const RUNTIME: &str = "0x949e3cc8727e6f0a8cd2734af1a44ce06cbd279749923868cad8b5bbe7dfd996";
pub const CREATION: &str = "0x56c86488af76ad8890b7365a0d008ba8ef28e588a617e75453093ac688457caf";
pub const COMPILER: &str = "0.8.20+commit.a1b79de6";
pub const SOURCE: &str = "TaggerToken.sol";
pub const NAME: &str = "TaggerToken";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const CACHE: &str = "out/ranks201-250-source-review/0x208bf3e7da9639f1eaefa2de78c23396b0682025.json";
pub const PRIMARY_GAP: &str = "No independently pinned public token repository was recovered. The single flattened TaggerToken.sol capture includes OpenZeppelin-like sections, not independently verified separate upstream dependency files.";
const BEFORE: &str = "0xa2646970667358221220705201f7eaedafb978a88e63d010c4cc81b619cc214770f5982cdcc0cc541de664736f6c63430008140033";
const AFTER: &str = "0xa264697066735822122011b527097fc43d381539cd1997ef7c0c50fc3120a4260198fc9c214d3a2d7fb664736f6c63430008140033";

pub fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}
pub fn root(n: u64) -> String {
    format!("0x{n:064x}")
}
fn bytes(v: &Value) -> Result<Vec<u8>> {
    Ok(hex::decode(v.as_str().context("hex string")?.trim_start_matches("0x"))?)
}
fn keccak(b: &[u8]) -> String {
    format!("0x{}", hex::encode(hash(b)))
}
pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE, "complete raw capture digest before parsing");
    let v = serde_json::from_slice(raw)?;
    verify_components(&v)?;
    Ok(v)
}
fn verify_components(v: &Value) -> Result<()> {
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&CONTRACT[2..])?,
        "chain/address binding"
    );
    for field in ["match", "runtimeMatch", "creationMatch"] {
        ensure!(v[field] == "match", "original {field} label");
    }
    ensure!(
        v["compilation"]
            == json!({"language":"Solidity","compiler":"solc","compilerVersion":COMPILER,"compilerSettings":{"libraries":{},"optimizer":{"runs":200,"enabled":true}},"name":NAME,"fullyQualifiedName":format!("{SOURCE}:{NAME}")}),
        "exact compiler identity/settings/target (default EVM target)"
    );
    ensure!(
        v["stdJsonInput"]["language"] == "Solidity" && v["stdJsonInput"]["settings"] == v["compilation"]["compilerSettings"],
        "compiler input settings"
    );
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == 1 && sources.contains_key(SOURCE) && v["stdJsonInput"]["sources"] == v["sources"],
        "one complete unmodified flattened source/input"
    );
    let source = v["sources"][SOURCE]["content"].as_str().context("source content")?;
    ensure!(sha(source.as_bytes()) == SOURCE_SHA, "exact flattened source bytes including line endings");
    let metadata = &v["metadata"];
    ensure!(
        metadata["sources"].as_object().context("source metadata")?.len() == 1 && metadata["sources"][SOURCE]["keccak256"] == keccak(source.as_bytes()),
        "complete metadata/source hash"
    );
    ensure!(
        metadata["compiler"]["version"] == COMPILER && metadata["language"] == "Solidity" && metadata["version"] == 1,
        "metadata compiler/language/version"
    );
    ensure!(
        metadata["settings"]
            == json!({"compilationTarget":{SOURCE:NAME},"evmVersion":"shanghai","libraries":{},"metadata":{"bytecodeHash":"ipfs"},"optimizer":{"enabled":true,"runs":200},"remappings":[]}),
        "compiler metadata resolved default target/settings"
    );
    ensure!(
        v["sourceIds"] == json!({SOURCE:{"id":0}}) && v["sourceIds"] == v["stdJsonOutput"]["sources"],
        "complete compiler source IDs"
    );
    let output = &v["stdJsonOutput"]["contracts"][SOURCE][NAME];
    let output_metadata: Value = serde_json::from_str(output["metadata"].as_str().context("compiler metadata")?)?;
    ensure!(output_metadata == *metadata, "saved compiler metadata binding");
    for field in ["abi", "devdoc", "userdoc", "storageLayout", "transientStorageLayout"] {
        ensure!(output[field] == v[field], "compiler output {field}");
    }
    ensure!(
        metadata["output"]["abi"] == v["abi"] && metadata["output"]["devdoc"] == v["devdoc"] && metadata["output"]["userdoc"] == v["userdoc"],
        "metadata ABI/docs"
    );
    let constructors: Vec<_> = v["abi"].as_array().context("ABI")?.iter().filter(|x| x["type"] == "constructor").collect();
    ensure!(constructors.len() == 1 && constructors[0]["inputs"] == json!([]), "no constructor arguments");
    ensure!(
        v["deployment"]
            == json!({"transactionHash":"0x28f64a9eaa0cd90124e232d338c99fe1420bab66614144c3dda58ec1c3b1829a","blockNumber":"44972322","transactionIndex":"52","deployer":"0xe096774FEF711A0b28Cb7bC9839b892BD5E2f603"}),
        "recorded deployment identity"
    );
    verify_bytecode(v, false)?;
    verify_bytecode(v, true)?;
    verify_layout(v)?;
    ensure!(
        identifiers(source).iter().filter(|s| s.as_str() == "_setRoleAdmin").count() == 4,
        "reviewed declaration plus constructor/public admin callsites"
    );
    for exact in [
        "_roles[role].members[account] = true;",
        "_roles[role].members[account] = false;",
        "_roles[role].adminRole = adminRole;",
        "function setRoleAdmin(bytes32 role, bytes32 adminRole) public onlyOwner {",
        "_setRoleAdmin(role, adminRole);",
        "_setRoleAdmin(ROLE_DEPLOYER, ROLE_DEPLOYER);",
        "_setRoleAdmin(ROLE_OPERATOR, ROLE_DEPLOYER);",
        "if (!hasRole(role, account) && owner() != account)",
        "return _balances[account];",
        "require(account == _msgSender(), \"AccessControl: can only renounce roles for self\");",
    ] {
        ensure!(source.contains(exact), "reviewed source statement {exact}");
    }
    Ok(())
}
// Only the recorded 53 bytes may change. In creation code they are embedded:
// the unchanged suffix after the CBOR block is retained and compared in full.
fn verify_bytecode(v: &Value, creation: bool) -> Result<()> {
    let (field, compiled_field, offset, length, digest) = if creation {
        ("creationBytecode", "bytecode", 13692usize, 13809usize, CREATION)
    } else {
        ("runtimeBytecode", "deployedBytecode", 11482, 11535, RUNTIME)
    };
    let saved = &v[field];
    let output = &v["stdJsonOutput"]["contracts"][SOURCE][NAME]["evm"][compiled_field];
    ensure!(saved["linkReferences"] == json!({}) && output["linkReferences"] == json!({}), "no links");
    if creation {
        ensure!(
            saved["immutableReferences"].is_null() && output["immutableReferences"].is_null(),
            "no creation immutable substitutions"
        );
    } else {
        ensure!(
            saved["immutableReferences"] == json!({}) && output["immutableReferences"] == json!({}),
            "no runtime immutables"
        );
    }
    ensure!(
        saved["cborAuxdata"] == json!({"1":{"value":BEFORE,"offset":offset}}),
        "exact original CBOR set/value/offset"
    );
    ensure!(
        saved["transformations"] == json!([{"id":"1","type":"replace","offset":offset,"reason":"cborAuxdata"}]),
        "sole exact CBOR transformation"
    );
    ensure!(
        saved["transformationValues"] == json!({"cborAuxdata":{"1":AFTER}}),
        "exact replacement set/value, no constructor append"
    );
    let mut compiled = bytes(&saved["recompiledBytecode"])?;
    ensure!(
        compiled.len() == length && compiled == bytes(&output["object"])?,
        "complete saved compiler bytes"
    );
    ensure!(saved["sourceMap"] == output["sourceMap"], "compiler source map");
    let before = bytes(&json!(BEFORE))?;
    let after = bytes(&json!(AFTER))?;
    ensure!(
        before.len() == 53 && after.len() == 53 && compiled.get(offset..offset + 53) == Some(before.as_slice()),
        "original CBOR bytes at exact location"
    );
    compiled[offset..offset + 53].copy_from_slice(&after);
    ensure!(
        compiled == bytes(&saved["onchainBytecode"])? && keccak(&compiled) == digest,
        "whole saved bytecode equality/hash after only declared replacement"
    );
    Ok(())
}
fn verify_layout(v: &Value) -> Result<()> {
    let layout = &v["storageLayout"];
    let fields = layout["storage"].as_array().context("layout storage")?;
    let names = [
        "_balances",
        "_allowances",
        "_totalSupply",
        "_name",
        "_symbol",
        "_status",
        "_roles",
        "_owner",
        "_signer",
        "_mode",
        "_positionId",
        "_lockId",
        "_claims",
        "_feeCollectors",
        "_launched",
    ];
    let types = [
        "t_mapping(t_address,t_uint256)",
        "t_mapping(t_address,t_mapping(t_address,t_uint256))",
        "t_uint256",
        "t_string_storage",
        "t_string_storage",
        "t_uint256",
        "t_mapping(t_bytes32,t_struct(RoleData)2408_storage)",
        "t_address",
        "t_address",
        "t_uint256",
        "t_uint256",
        "t_uint256",
        "t_mapping(t_address,t_bool)",
        "t_mapping(t_address,t_bool)",
        "t_bool",
    ];
    ensure!(fields.len() == names.len(), "complete layout");
    for (i, (name, ty)) in names.iter().zip(types).enumerate() {
        let expected_slot = i.to_string();
        ensure!(
            fields[i]["slot"] == expected_slot
                && fields[i]["offset"] == 0
                && fields[i]["label"] == *name
                && fields[i]["type"] == ty
                && fields[i]["contract"] == format!("{SOURCE}:{NAME}"),
            "layout slot/type/name/offset/contract {i}"
        );
    }
    let types = &layout["types"];
    let role = &types["t_mapping(t_bytes32,t_struct(RoleData)2408_storage)"];
    ensure!(
        role["encoding"] == "mapping" && role["key"] == "t_bytes32" && role["value"] == "t_struct(RoleData)2408_storage",
        "outer arbitrary bytes32 mapping"
    );
    let record = &types["t_struct(RoleData)2408_storage"];
    let members = record["members"].as_array().context("role members")?;
    ensure!(
        record["encoding"] == "inplace" && record["numberOfBytes"] == "64" && members.len() == 2,
        "two-word role record"
    );
    for (i, (label, ty)) in [("members", "t_mapping(t_address,t_bool)"), ("adminRole", "t_bytes32")].iter().enumerate() {
        let expected_slot = i.to_string();
        ensure!(
            members[i]["slot"] == expected_slot
                && members[i]["offset"] == 0
                && members[i]["label"] == *label
                && members[i]["type"] == *ty
                && members[i]["contract"] == format!("{SOURCE}:{NAME}"),
            "exact role record member"
        );
    }
    let membership = &types["t_mapping(t_address,t_bool)"];
    ensure!(
        membership["encoding"] == "mapping" && membership["key"] == "t_address" && membership["value"] == "t_bool",
        "address/bool membership"
    );
    ensure!(
        types["t_bool"]["numberOfBytes"] == "1" && types["t_address"]["numberOfBytes"] == "20" && types["t_bytes32"]["numberOfBytes"] == "32",
        "primitive widths"
    );
    Ok(())
}
fn identifiers(source: &str) -> Vec<String> {
    let b = source.as_bytes();
    let mut at = 0;
    let mut out = Vec::new();
    while at < b.len() {
        if b[at..].starts_with(b"//") {
            while at < b.len() && b[at] != b'\n' {
                at += 1;
            }
        } else if b[at..].starts_with(b"/*") {
            at += 2;
            while at < b.len() && !b[at..].starts_with(b"*/") {
                at += 1;
            }
            at = (at + 2).min(b.len());
        } else if matches!(b[at], b'\'' | b'"') {
            let q = b[at];
            at += 1;
            while at < b.len() {
                if b[at] == b'\\' {
                    at = (at + 2).min(b.len());
                } else {
                    let end = b[at] == q;
                    at += 1;
                    if end {
                        break;
                    }
                }
            }
        } else if b[at].is_ascii_alphabetic() || b[at] == b'_' {
            let start = at;
            at += 1;
            while at < b.len() && (b[at].is_ascii_alphanumeric() || b[at] == b'_') {
                at += 1;
            }
            out.push(source[start..at].into());
        } else {
            at += 1;
        }
    }
    out
}

pub fn candidate(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == BASELINE, "unchanged full baseline digest");
    let baseline: Value = serde_json::from_slice(raw)?;
    let mut c = baseline
        .as_array()
        .context("baseline array")?
        .iter()
        .find(|p| p["contract"] == CONTRACT)
        .context("Tagger baseline")?
        .clone();
    ensure!(c["code_hash"] == RUNTIME && c["balance_slot"] == root(0), "baseline runtime/balance binding");
    ensure!(
        c["other_mapping_words"].as_object_mut().context("legacy mapping")?.remove(&root(6)) == Some(json!(2)),
        "legacy root6 width2 required"
    );
    ensure!(c.get("other_mapping_paths").is_none(), "preexisting paths");
    c["other_mapping_paths"] = json!([
        {"root":root(6),"key_types":["bytes32"],"offset":1,"words":1},
        {"root":root(6),"key_types":["bytes32","address"],"offset":0,"words":1}
    ]);
    Ok(json!([c]))
}
pub fn verify_candidate(baseline: &[u8], value: &Value) -> Result<()> {
    ensure!(candidate(baseline)? == *value, "only exact outer admin and inner membership rules may change");
    Ok(())
}
pub fn review(v: &Value) -> Result<Value> {
    verify_components(v)?;
    Ok(json!({
        "qualified":false,"contract":CONTRACT,"chain_id":56,"capture_sha256":CAPTURE,
        "source_files":[{"path":SOURCE,"sha256":SOURCE_SHA,"keccak256":v["metadata"]["sources"][SOURCE]["keccak256"]}],
        "independent_primary_source_gap":PRIMARY_GAP,"independently_verified_upstream_files":0,
        "compiler":COMPILER,"compiler_input_evm_target":"unspecified (compiler default)","compiler_metadata_evm_target":"shanghai",
        "optimizer":{"enabled":true,"runs":200},"original_match_labels":{"match":"match","runtimeMatch":"match","creationMatch":"match"},
        "runtime":{"bytes":11535,"keccak256":RUNTIME,"cbor_offset":11482},"creation":{"bytes":13809,"keccak256":CREATION,"cbor_offset":13692,"constructor_arguments":0,"unchanged_suffix_bytes":64},
        "sole_transformation_per_bytecode":{"kind":"53-byte CBOR replacement","before":BEFORE,"after":AFTER},"immutable_or_link_substitutions":0,
        "saved_compiler_reconstruction":true,"fresh_compilation":false,"deployment":v["deployment"],
        "role_root":root(6),"balance_root":root(0),"role_admin_identifier_references":4,
        "admin_reachability":"Two constructor assignments plus public onlyOwner setRoleAdmin(bytes32,bytes32) permit arbitrary outer role admin words.",
        "membership_reachability":"Inherited grant/revoke accept arbitrary roles; BasicAccessControl._checkRole also permits owner without membership. Renounce requires account == caller.",
        "scope":"Saved source/compiler/bytecode binding and synthetic storage shapes. Not executed authorization, current owner state, producer visibility or deployment/package qualification."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: &[u8] = include_bytes!("../../tests/fixtures/tagger-role-candidate/TaggerToken.json");
    fn good() -> Value {
        verify_capture(RAW).unwrap()
    }
    fn change_hex(v: &mut Value, field: &str, index: usize) {
        let mut b = bytes(&v[field]).unwrap();
        b[index] ^= 1;
        v[field] = json!(format!("0x{}", hex::encode(b)));
    }
    #[test]
    fn tagger_complete_capture_source_compiler_layout_and_metadata_bindings() {
        let good = good();
        let mut raw = RAW.to_vec();
        raw.push(b' ');
        assert!(verify_capture(&raw).is_err());
        for (pointer, value) in [
            ("/chainId", json!("1")),
            ("/address", json!("0x01")),
            ("/match", json!("exact_match")),
            ("/runtimeMatch", json!("exact_match")),
            ("/creationMatch", json!("exact_match")),
            ("/sources", json!({})),
            ("/stdJsonInput/sources", json!({})),
            ("/sources/TaggerToken.sol/content", json!("modified public token body")),
            ("/compilation/compilerVersion", json!("0.8.21")),
            ("/compilation/compilerSettings/optimizer/runs", json!(201)),
            ("/stdJsonInput/settings/optimizer/runs", json!(201)),
            ("/metadata/compiler/version", json!("changed")),
            ("/metadata/settings/evmVersion", json!("paris")),
            ("/sourceIds", json!({})),
            ("/stdJsonOutput/sources", json!({})),
            ("/storageLayout/storage/6/slot", json!("7")),
            ("/storageLayout/types/t_struct(RoleData)2408_storage/members/1/slot", json!("2")),
            ("/storageLayout/types/t_mapping(t_address,t_bool)/key", json!("t_bytes32")),
            ("/deployment/blockNumber", json!("44972323")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).expect(pointer) = value;
            assert!(verify_components(&bad).is_err(), "{pointer}");
        }
        // A coordinated source/input/metadata edit cannot substitute another
        // flattened body even if its self-reported hashes agree.
        let mut bad = good.clone();
        bad["sources"][SOURCE]["content"] = json!("modified");
        bad["stdJsonInput"]["sources"] = bad["sources"].clone();
        bad["metadata"]["sources"][SOURCE]["keccak256"] = json!(keccak(b"modified"));
        assert!(verify_components(&bad).is_err());
        // The compiler input never specified an EVM version. Its resolved
        // metadata says Shanghai; adding an explicit target changes evidence.
        let mut bad = good.clone();
        bad["compilation"]["compilerSettings"]["evmVersion"] = json!("shanghai");
        bad["stdJsonInput"]["settings"] = bad["compilation"]["compilerSettings"].clone();
        assert!(verify_components(&bad).is_err());
    }
    #[test]
    fn tagger_only_exact_cbor_substitution_set_is_permitted() {
        let good = good();
        for (field, creation) in [("runtimeBytecode", false), ("creationBytecode", true)] {
            for (pointer, value) in [
                ("/transformations", json!([])),
                ("/transformations", json!([{}, {}])),
                ("/transformations/0/id", json!("2")),
                ("/transformations/0/type", json!("insert")),
                ("/transformations/0/reason", json!("library")),
                ("/transformations/0/offset", json!(0)),
                ("/cborAuxdata/1/offset", json!(usize::MAX)),
                ("/cborAuxdata/1/value", json!(AFTER)),
                ("/transformationValues/cborAuxdata/1", json!(BEFORE)),
                ("/linkReferences", json!({"unreviewed":[]})),
            ] {
                let mut bad = good.clone();
                *bad[field].pointer_mut(pointer).expect(pointer) = value;
                assert!(verify_bytecode(&bad, creation).is_err(), "{field}{pointer}");
            }
            for extra in ["constructorArguments", "immutableReferences", "library"] {
                let mut bad = good.clone();
                bad[field]["transformationValues"][extra] = json!({});
                assert!(verify_bytecode(&bad, creation).is_err(), "{field} extra {extra}");
            }
            let mut bad = good.clone();
            bad[field]["immutableReferences"] = json!({"1":[{"start":0,"length":32}]});
            assert!(verify_bytecode(&bad, creation).is_err());
            let mut bad = good.clone();
            let patch = bad[field]["transformations"][0].clone();
            bad[field]["transformations"].as_array_mut().unwrap().push(patch);
            assert!(verify_bytecode(&bad, creation).is_err(), "duplicate/overlapping patch");
            let mut bad = good.clone();
            bad[field]["cborAuxdata"]["2"] = bad[field]["cborAuxdata"]["1"].clone();
            assert!(verify_bytecode(&bad, creation).is_err(), "extra metadata site");
        }
    }
    #[test]
    fn tagger_whole_bytecode_including_creation_suffix_is_bound() {
        let good = good();
        for (field, output_field, creation, offset, len) in [
            ("runtimeBytecode", "deployedBytecode", false, 11482, 11535),
            ("creationBytecode", "bytecode", true, 13692, 13809),
        ] {
            for index in [0, offset - 1, offset, offset + 52, len - 1] {
                let mut bad = good.clone();
                change_hex(&mut bad[field], "onchainBytecode", index);
                assert!(verify_bytecode(&bad, creation).is_err(), "changed saved {field} byte {index}");
                let mut bad = good.clone();
                change_hex(&mut bad[field], "recompiledBytecode", index);
                // Even a coordinated compiler-output mutation cannot change
                // any code byte or the exact original metadata block.
                bad["stdJsonOutput"]["contracts"][SOURCE][NAME]["evm"][output_field]["object"] = bad[field]["recompiledBytecode"].clone();
                assert!(verify_bytecode(&bad, creation).is_err(), "changed compiler {field} byte {index}");
            }
            let mut bad = good.clone();
            let shorter = bytes(&bad[field]["onchainBytecode"]).unwrap()[..offset + 53].to_vec();
            if creation {
                bad[field]["onchainBytecode"] = json!(format!("0x{}", hex::encode(shorter)));
                assert!(verify_bytecode(&bad, creation).is_err(), "creation suffix may not be truncated");
            }
        }
        let c = bytes(&good["creationBytecode"]["onchainBytecode"]).unwrap();
        assert_eq!(c.len() - (13692 + 53), 64);
    }
    #[test]
    fn tagger_candidate_changes_only_the_two_exact_paths_and_preserves_primary_gap() {
        let raw = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let candidate: Value = serde_json::from_str(include_str!("../../tests/fixtures/tagger-role-candidate/layouts.json")).unwrap();
        verify_candidate(raw, &candidate).unwrap();
        let saved: Value = serde_json::from_str(include_str!("../../tests/fixtures/tagger-role-candidate/source-review.json")).unwrap();
        assert_eq!(review(&good()).unwrap(), saved);
        assert_eq!(saved["qualified"], false);
        assert_eq!(saved["independent_primary_source_gap"], PRIMARY_GAP);
        assert_eq!(saved["independently_verified_upstream_files"], 0);
        for (pointer, value) in [
            ("/0/code_hash", json!("0x00")),
            ("/0/balance_slot", json!(root(9))),
            ("/0/other_slots", json!([])),
            ("/0/other_mapping_slots", json!([])),
            ("/0/other_mapping_paths", json!([])),
            ("/0/other_mapping_paths/0/key_types", json!(["address"])),
            ("/0/other_mapping_paths/0/offset", json!(0)),
            ("/0/other_mapping_paths/0/words", json!(2)),
            ("/0/other_mapping_paths/1/words", json!(2)),
        ] {
            let mut bad = candidate.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_candidate(raw, &bad).is_err(), "{pointer}");
        }
        let mut changed = raw.to_vec();
        changed.push(b' ');
        assert!(verify_candidate(&changed, &candidate).is_err());
    }
}
