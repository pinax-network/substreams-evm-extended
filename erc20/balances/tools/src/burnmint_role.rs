//! Exact source controls for one unqualified BurnMint role-path candidate.
use anyhow::{ensure, Context, Result};
use erc20_balances::hash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const CONTRACT: &str = "0xac23b90a79504865d52b49b327328411a23d4db2";
pub const CAPTURE: &str = "8c2bf80a6aef279bc19fbb13885c1067247b9fd8b2d6cf3a0e0645b7bd4fd489";
pub const RUNTIME: &str = "0x688f1e2193eea752e7eaa2acc9607ede95429383234670d6d84c0ee3336eb3d0";
pub const BASELINE: &str = "e503fae553adf6800b714bea95234c930df2dec21d0727bdd36f51a891734468";
pub const PIN: &str = "e6287bd5ec0cb86925584b050b6aed0fa9b1b2e8";
pub const PREFIX: &str = "node_modules/@chainlink/contracts/";
pub const ROOT: &str = "0x0000000000000000000000000000000000000000000000000000000000000005";
const TOKEN: &str = "src/v0.8/shared/token/ERC20/BurnMintERC20.sol";
const INTERFACE: &str = "src/v0.8/shared/interfaces/IGetCCIPAdmin.sol";

pub fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn bytes(value: &Value) -> Result<Vec<u8>> {
    Ok(hex::decode(value.as_str().context("hex string")?.trim_start_matches("0x"))?)
}

/// Bind the complete original capture before trusting any contained evidence.
pub fn verify_capture(raw: &[u8]) -> Result<Value> {
    ensure!(sha(raw) == CAPTURE, "original source capture digest changed");
    let capture: Value = serde_json::from_slice(raw)?;
    verify_components(&capture)?;
    Ok(capture)
}

fn verify_components(v: &Value) -> Result<()> {
    ensure!(
        v["chainId"] == "56" && bytes(&v["address"])? == hex::decode(&CONTRACT[2..])?,
        "source address/network binding"
    );
    ensure!(v["runtimeMatch"] == "match", "source runtime match status");
    let sources = v["sources"].as_object().context("sources")?;
    let metadata = v["metadata"]["sources"].as_object().context("source metadata")?;
    ensure!(sources.len() == 14 && sources.keys().eq(metadata.keys()), "complete 14-source set required");
    for (name, value) in sources {
        ensure!(name.starts_with(PREFIX), "unexpected source path");
        let content = value["content"].as_str().context("source content")?;
        ensure!(
            format!("0x{}", hex::encode(hash(content.as_bytes()))) == metadata[name]["keccak256"],
            "source hash: {name}"
        );
    }
    let runtime = &v["runtimeBytecode"];
    ensure!(
        runtime["immutableReferences"] == json!({"54":[{"start":3213,"length":32}],"57":[{"start":439,"length":32},{"start":2357,"length":32}]}),
        "immutable reference schema"
    );
    ensure!(
        runtime["transformationValues"]
            == json!({"immutables":{"54":"0x0000000000000000000000000000000000000000000000000000000000000012","57":"0x0000000000000000000000000000000000000000204fce5e3e25026110000000"}}),
        "immutable values"
    );
    ensure!(
        runtime["transformations"]
            == json!([
                {"id":"54","offset":3213,"reason":"immutable","type":"replace"},
                {"id":"57","offset":439,"reason":"immutable","type":"replace"},
                {"id":"57","offset":2357,"reason":"immutable","type":"replace"}
            ]),
        "exact immutable replacements"
    );
    ensure!(
        runtime["cborAuxdata"] == json!({}) && runtime["linkReferences"] == json!({}),
        "unreviewed bytecode dependencies"
    );
    let mut compiled = bytes(&runtime["recompiledBytecode"])?;
    for (id, offset) in [("54", 3213), ("57", 439), ("57", 2357)] {
        compiled
            .get_mut(offset..offset + 32)
            .context("immutable bytecode range")?
            .copy_from_slice(&bytes(&runtime["transformationValues"]["immutables"][id])?);
    }
    let deployed = bytes(&runtime["onchainBytecode"])?;
    ensure!(
        compiled == deployed && format!("0x{}", hex::encode(hash(&deployed))) == RUNTIME,
        "saved runtime reconstruction"
    );
    let layout = &v["storageLayout"];
    let fields = layout["storage"].as_array().context("layout fields")?;
    ensure!(fields.len() == 7, "full storage layout required");
    for (i, (slot, name)) in [
        ("0", "_balances"),
        ("1", "_allowances"),
        ("2", "_totalSupply"),
        ("3", "_name"),
        ("4", "_symbol"),
        ("5", "_roles"),
        ("6", "s_ccipAdmin"),
    ]
    .iter()
    .enumerate()
    {
        ensure!(
            fields[i]["slot"] == *slot && fields[i]["label"] == *name && fields[i]["offset"] == 0,
            "layout slot/name/offset"
        );
    }
    let role = &layout["types"][fields[5]["type"].as_str().context("role type")?];
    ensure!(role["encoding"] == "mapping" && role["key"] == "t_bytes32", "outer role mapping");
    let record = &layout["types"][role["value"].as_str().context("role record")?];
    let members = record["members"].as_array().context("role members")?;
    ensure!(
        record["numberOfBytes"] == "64"
            && members.len() == 2
            && members[0]["slot"] == "0"
            && members[0]["offset"] == 0
            && members[0]["label"] == "members"
            && members[1]["slot"] == "1"
            && members[1]["offset"] == 0
            && members[1]["type"] == "t_bytes32"
            && members[1]["label"] == "adminRole",
        "role record shape"
    );
    let member = &layout["types"][members[0]["type"].as_str().context("membership type")?];
    ensure!(
        member["encoding"] == "mapping" && member["key"] == "t_address" && member["value"] == "t_bool",
        "inner boolean membership"
    );
    Ok(())
}

pub fn primary_url(name: &str) -> Result<String> {
    let suffix = name.strip_prefix(PREFIX).context("primary source prefix")?;
    let path = if suffix == INTERFACE {
        "src/v0.8/ccip/interfaces/IGetCCIPAdmin.sol"
    } else {
        suffix
    };
    Ok(format!("https://raw.githubusercontent.com/smartcontractkit/chainlink/{PIN}/contracts/{path}"))
}

/// Only the token's one import relocation is allowed; all other bytes match.
pub fn verify_primary(capture: &Value, primary: &Value) -> Result<()> {
    ensure!(primary["pin"] == PIN && primary["qualified"] == false, "primary pin or qualification");
    let sources = capture["sources"].as_object().context("sources")?;
    let saved = primary["sources"].as_object().context("primary sources")?;
    ensure!(sources.len() == 14 && sources.keys().eq(saved.keys()), "complete primary source set");
    for (name, source) in sources {
        let p = &saved[name];
        let content = p["content"].as_str().context("primary content")?;
        ensure!(
            p["url"] == primary_url(name)? && p["sha256"] == sha(content.as_bytes()),
            "primary source URL/hash"
        );
        let normalized = if name == &format!("{PREFIX}{TOKEN}") {
            let old = "../../../ccip/interfaces/IGetCCIPAdmin.sol";
            ensure!(content.matches(old).count() == 1, "exactly one original import required");
            content.replacen(old, "../../../shared/interfaces/IGetCCIPAdmin.sol", 1)
        } else {
            content.to_owned()
        };
        ensure!(source["content"] == normalized, "unreviewed primary source difference: {name}");
    }
    Ok(())
}

pub fn candidate(baseline_raw: &[u8]) -> Result<Value> {
    ensure!(sha(baseline_raw) == BASELINE, "qualified431 baseline changed");
    let baseline: Value = serde_json::from_slice(baseline_raw)?;
    let mut candidate = baseline
        .as_array()
        .context("baseline")?
        .iter()
        .find(|p| p["contract"] == CONTRACT)
        .context("original profile")?
        .clone();
    ensure!(
        candidate["other_mapping_words"].as_object_mut().context("legacy role width")?.remove(ROOT) == Some(json!(2)),
        "legacy root5 width2 required"
    );
    candidate["other_mapping_paths"] = json!([{"root":ROOT,"key_types":["bytes32","address"],"offset":0,"words":1}]);
    Ok(json!([candidate]))
}

pub fn verify_candidate(baseline_raw: &[u8], candidate_json: &Value) -> Result<()> {
    ensure!(candidate(baseline_raw)? == *candidate_json, "only the single reviewed profile/rule may change");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: &[u8] = include_bytes!("../../tests/fixtures/burnmint-role-candidate/source-capture.json");
    #[test]
    fn complete_capture_and_component_tampering_is_rejected() {
        let good = verify_capture(RAW).unwrap();
        let mut changed_raw = RAW.to_vec();
        changed_raw.push(b' ');
        assert!(verify_capture(&changed_raw).is_err());
        for (pointer, value) in [
            ("/chainId", json!("1")),
            ("/address", json!("0x0000000000000000000000000000000000000001")),
            ("/sources", json!({})),
            ("/runtimeMatch", json!("partial")),
            ("/runtimeBytecode/transformations", json!([])),
            ("/runtimeBytecode/immutableReferences/54/0/start", json!(3214)),
            ("/runtimeBytecode/transformationValues/immutables/54", json!("0x00")),
            ("/runtimeBytecode/recompiledBytecode", json!("0x00")),
            ("/runtimeBytecode/onchainBytecode", json!("0x00")),
            ("/storageLayout/storage/5/slot", json!("6")),
            ("/storageLayout/types/t_mapping(t_address,t_bool)/key", json!("t_bytes32")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).expect(pointer) = value;
            assert!(verify_components(&bad).is_err(), "accepted tamper at {pointer}");
        }
    }

    #[test]
    fn primary_sources_allow_only_the_documented_import_relocation() {
        let capture = verify_capture(RAW).unwrap();
        let primary: Value = serde_json::from_str(include_str!("../../tests/fixtures/burnmint-role-candidate/primary-sources.json")).unwrap();
        verify_primary(&capture, &primary).unwrap();
        for (pointer, value) in [("/pin", json!("main")), ("/qualified", json!(true)), ("/sources", json!({}))] {
            let mut bad = primary.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_primary(&capture, &bad).is_err());
        }
        for name in primary["sources"].as_object().unwrap().keys() {
            let mut bad = primary.clone();
            bad["sources"][name]["content"] = json!(format!("{}\n// tamper", bad["sources"][name]["content"].as_str().unwrap()));
            // Even a self-consistent replacement digest cannot authorize new code.
            bad["sources"][name]["sha256"] = json!(sha(bad["sources"][name]["content"].as_str().unwrap().as_bytes()));
            assert!(verify_primary(&capture, &bad).is_err(), "primary source {name}");
        }
    }

    #[test]
    fn only_one_candidate_rule_can_change_and_published_profiles_stay_bound() {
        let baseline = include_bytes!("../../tests/fixtures/bsc-refined450-layouts.json");
        let candidate: Value = serde_json::from_str(include_str!("../../tests/fixtures/burnmint-role-candidate/layouts.json")).unwrap();
        verify_candidate(baseline, &candidate).unwrap();
        for (pointer, value) in [
            ("/0/code_hash", json!("0x00")),
            ("/0/other_mapping_paths/0/words", json!(2)),
            ("/0/other_mapping_paths/0/root", json!("0x06")),
            ("/0/other_mapping_paths/0/key_types", json!(["address", "bytes32"])),
        ] {
            let mut bad = candidate.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(verify_candidate(baseline, &bad).is_err());
        }
        let mut changed_baseline = baseline.to_vec();
        changed_baseline.push(b' ');
        assert!(verify_candidate(&changed_baseline, &candidate).is_err());
    }
}
