//! Checks saved compiler output against complete pinned captures. This reconstructs
//! captured runtime bytes; it does not compile sources or qualify deployment state.
use super::binding::CAPTURES;
use super::{address, Address};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn hexbytes(v: &Value) -> Result<Vec<u8>> {
    let s = v.as_str().context("hex text")?;
    Ok(hex::decode(s.strip_prefix("0x").unwrap_or(s))?)
}
fn selected(v: &Value) -> Result<&Value> {
    let (path, name) = v["compilation"]["fullyQualifiedName"]
        .as_str()
        .context("compiler target")?
        .rsplit_once(':')
        .context("qualified target")?;
    let output = &v["stdJsonOutput"]["contracts"][path][name];
    ensure!(output.is_object(), "missing selected compiler output");
    Ok(output)
}
fn text<'a>(v: &'a Value, path: &str) -> Result<&'a str> {
    v["sources"][path]["content"].as_str().context("complete source text")
}
fn field(v: &Value, name: &str, slot: u64, offset: u64, ty: &str) -> Result<()> {
    let rows: Vec<_> = v["storageLayout"]["storage"]
        .as_array()
        .context("storage layout")?
        .iter()
        .filter(|r| r["label"] == name)
        .collect();
    ensure!(rows.len() == 1, "missing/duplicate selected field {name}");
    let row = rows[0];
    ensure!(
        row["slot"].as_str() == Some(slot.to_string().as_str()) && row["offset"] == offset && row["type"] == ty,
        "selected layout field differs: {name}"
    );
    Ok(())
}
/// Public for adversarial semantic controls. Callers admitting a binding must
/// additionally use verify_capture, which first checks the complete raw digest.
pub fn verify(v: &Value, index: usize) -> Result<()> {
    let (_, contract, _, raw) = CAPTURES.get(index).context("capture index")?;
    let frozen: Value = serde_json::from_slice(raw)?;
    ensure!(
        address(v["address"].as_str().context("address")?)? == address(contract)? && v["chainId"] == frozen["chainId"],
        "capture identity"
    );
    let output = selected(v)?;
    ensure!(v["storageLayout"] == output["storageLayout"], "compiler/layout disagreement");
    let metadata: Value = serde_json::from_str(output["metadata"].as_str().context("compiler metadata")?)?;
    ensure!(
        metadata == v["metadata"] && metadata["settings"] == frozen["metadata"]["settings"],
        "compiler metadata/settings binding"
    );
    ensure!(v["stdJsonInput"]["settings"] == frozen["stdJsonInput"]["settings"], "input settings differ");
    let sources = v["sources"].as_object().context("sources")?;
    ensure!(
        sources.len() == metadata["sources"].as_object().context("metadata sources")?.len() && v["sources"] == v["stdJsonInput"]["sources"],
        "complete compiler source closure"
    );
    for (path, entry) in sources {
        let content = entry["content"].as_str().context("source content")?;
        ensure!(
            metadata["sources"][path]["keccak256"] == format!("0x{}", hex::encode(erc20_balances::hash(content.as_bytes()))),
            "source Keccak differs: {path}"
        );
        ensure!(v["sourceIds"][path] == v["stdJsonOutput"]["sources"][path], "source ID binding differs");
    }
    // The exact original schema is closed, including all patch IDs/sites/values.
    // The following reconstruction independently consumes every declared patch.
    let r = &v["runtimeBytecode"];
    for name in [
        "immutableReferences",
        "linkReferences",
        "transformations",
        "transformationValues",
        "cborAuxdata",
    ] {
        ensure!(r[name] == frozen["runtimeBytecode"][name], "runtime {name} differs from closed schema");
    }
    ensure!(r["linkReferences"] == json!({}), "linked runtime unsupported");
    let evm = &output["evm"]["deployedBytecode"];
    for name in ["immutableReferences", "linkReferences", "sourceMap"] {
        ensure!(r[name] == evm[name], "saved compiler runtime {name} differs");
    }
    let mut code = hexbytes(&r["recompiledBytecode"])?;
    ensure!(code == hexbytes(&evm["object"])? && !code.is_empty(), "saved runtime compiler bytes differ");
    let mut expected = Vec::new();
    let mut touched = BTreeSet::new();
    let refs = r["immutableReferences"].as_object().context("immutable references")?;
    ensure!(
        r["transformationValues"]["immutables"].as_object().map_or(0, |v| v.len()) == refs.len(),
        "immutable value closure"
    );
    for (id, sites) in refs {
        let replacement = hexbytes(&r["transformationValues"]["immutables"][id])?;
        ensure!(replacement.len() == 32, "immutable word width");
        for site in sites.as_array().context("immutable sites")? {
            let start = site["start"].as_u64().context("immutable start")? as usize;
            ensure!(site["length"] == 32 && start + 32 <= code.len(), "immutable patch bounds");
            ensure!(
                code[start..start + 32] == [0; 32] && (start..start + 32).all(|i| touched.insert(i)),
                "immutable placeholder/overlap"
            );
            code[start..start + 32].copy_from_slice(&replacement);
            expected.push(json!({"id":id,"type":"replace","offset":start,"reason":"immutable"}));
        }
    }
    let aux = r["cborAuxdata"].as_object().context("CBOR schema")?;
    for (id, meta) in aux {
        let old = hexbytes(&meta["value"])?;
        let offset = meta["offset"].as_u64().context("CBOR offset")? as usize;
        ensure!(offset + old.len() == code.len() && code[offset..] == old, "exact compiler CBOR suffix");
        if let Some(new) = r["transformationValues"]["cborAuxdata"].get(id) {
            let new = hexbytes(new)?;
            ensure!(
                new.len() == old.len() && (offset..offset + new.len()).all(|i| touched.insert(i)),
                "CBOR width/overlap"
            );
            code[offset..].copy_from_slice(&new);
            expected.push(json!({"id":id,"type":"replace","offset":offset,"reason":"cborAuxdata"}));
        }
    }
    let mut actual = r["transformations"].as_array().context("runtime transformations")?.clone();
    expected.sort_by_key(|x| x["offset"].as_u64());
    actual.sort_by_key(|x| x["offset"].as_u64());
    ensure!(
        expected == actual && code == hexbytes(&r["onchainBytecode"])?,
        "complete captured runtime reconstruction"
    );
    for (ty, label, width) in [("t_uint256", "uint256", "32"), ("t_address", "address", "20")] {
        ensure!(
            v["storageLayout"]["types"][ty] == json!({"label":label,"encoding":"inplace","numberOfBytes":width}),
            "scalar type binding"
        );
    }
    for (ty, value) in [("t_mapping(t_address,t_uint256)", "t_uint256"), ("t_mapping(t_address,t_bool)", "t_bool")] {
        if !v["storageLayout"]["types"][ty].is_null() {
            ensure!(
                v["storageLayout"]["types"][ty]["key"] == "t_address"
                    && v["storageLayout"]["types"][ty]["value"] == value
                    && v["storageLayout"]["types"][ty]["encoding"] == "mapping",
                "mapping type binding"
            );
        }
    }
    match index {
        0 | 1 => {
            field(v, "_balances", 0, 0, "t_mapping(t_address,t_uint256)")?;
            if index == 1 {
                for (name, slot) in [
                    ("_totalSupply", 2),
                    ("staticAccPerShare", 7),
                    ("nodeAccPerShare", 8),
                    ("totalNodeCount", 12),
                    ("accountedEmission", 17),
                ] {
                    field(v, name, slot, 0, "t_uint256")?;
                }
                for (name, slot) in [("userIndex", 9), ("userNodeIndex", 10)] {
                    field(v, name, slot, 0, "t_mapping(t_address,t_uint256)")?;
                }
                field(v, "isNode", 11, 0, "t_mapping(t_address,t_bool)")?;
                for (name, offset, ty, width) in [
                    ("tradingOpened", 0, "t_bool", "1"),
                    ("openTime", 1, "t_uint64", "8"),
                    ("lastEmissionUpdate", 9, "t_uint64", "8"),
                    ("lastTickRLbp", 17, "t_uint112", "14"),
                ] {
                    field(v, name, 6, offset, ty)?;
                    ensure!(
                        v["storageLayout"]["types"][ty]["numberOfBytes"] == width && v["storageLayout"]["types"][ty]["encoding"] == "inplace",
                        "packed lane binding"
                    );
                }
            }
        }
        2 | 3 => {
            let shift = if index == 2 { 2 } else { 0 };
            for (name, root, ty) in [
                ("_rOwned", 1, "t_mapping(t_address,t_uint256)"),
                ("_tOwned", 2, "t_mapping(t_address,t_uint256)"),
                ("_isExcluded", 4, "t_mapping(t_address,t_bool)"),
                ("_excluded", 5, "t_array(t_address)dyn_storage"),
                ("_tTotal", 6, "t_uint256"),
                ("_rTotal", 7, "t_uint256"),
            ] {
                let extra = if index == 2 && root >= 4 { 1 } else { 0 };
                field(v, name, root + shift + extra, 0, ty)?;
            }
            ensure!(
                v["storageLayout"]["types"]["t_array(t_address)dyn_storage"]
                    == json!({"base":"t_address","label":"address[]","encoding":"dynamic_array","numberOfBytes":"32"}),
                "ordered exclusion array type"
            );
        }
        _ => anyhow::bail!("capture index"),
    }
    // IDs are the saved compiler's immutable-reference IDs. The captures have
    // no AST; selected source declaration anchors are checked explicitly rather
    // than inventing an AST-to-ID or a new compiler-execution claim.
    let eip = "lib/openzeppelin-contracts/contracts/utils/cryptography/EIP712.sol";
    let declarations: Vec<(&str, &str, &str)> = match index {
        0 => vec![
            ("5089", eip, "bytes32 private immutable _cachedDomainSeparator;"),
            ("5091", eip, "uint256 private immutable _cachedChainId;"),
            ("5093", eip, "address private immutable _cachedThis;"),
            ("5095", eip, "bytes32 private immutable _hashedName;"),
            ("5097", eip, "bytes32 private immutable _hashedVersion;"),
            ("5100", eip, "ShortString private immutable _name;"),
            ("5103", eip, "ShortString private immutable _version;"),
            ("17581", "src/LBP.sol", "bytes32 internal immutable openingHash;"),
            ("17583", "src/LBP.sol", "address public immutable devWallet;"),
            ("17586", "src/LBP.sol", "IERC20 public immutable USDT;"),
            ("17589", "src/LBP.sol", "IPancakeFactory public immutable PANCAKE_FACTORY;"),
            ("17591", "src/LBP.sol", "address public immutable pair;"),
            ("17594", "src/LBP.sol", "BurnVault public immutable burnVault;"),
            ("17597", "src/LBP.sol", "RefVault public immutable refVault;"),
            ("17600", "src/LBP.sol", "PolVault public immutable polVault;"),
            ("17603", "src/LBP.sol", "IFomoVault public immutable fomoVault;"),
            ("17607", "src/LBP.sol", "LBPHashrate public immutable hashrate;"),
            ("17610", "src/LBP.sol", "address public immutable PANCAKE_ROUTER;"),
        ],
        1 => vec![
            ("9180", "src/LBPHashrate.sol", "ILBP public immutable lbp;"),
            ("9184", "src/LBPHashrate.sol", "IPancakePair public immutable pair;"),
            ("9187", "src/LBPHashrate.sol", "address public immutable refVault;"),
        ],
        2 => vec![
            ("1407", "CoinToken.sol", "IUniswapV2Router02 public immutable uniswapV2Router;"),
            ("1409", "CoinToken.sol", "address public immutable uniswapV2Pair;"),
        ],
        _ => vec![],
    };
    ensure!(declarations.len() == refs.len(), "immutable declaration closure");
    for (id, path, anchor) in declarations {
        ensure!(
            refs.contains_key(id) && text(v, path)?.matches(anchor).count() == 1,
            "immutable source declaration binding {id}"
        );
    }
    Ok(())
}
fn immutable_address(v: &Value, id: &str) -> Result<Address> {
    let bytes = hexbytes(&v["runtimeBytecode"]["transformationValues"]["immutables"][id])?;
    ensure!(bytes.len() == 32 && bytes[..12] == [0; 12], "immutable address width/padding");
    Ok(bytes[12..].try_into().unwrap())
}
pub fn lbp_exemptions(lbp: &Value, hlbp: &Value) -> Result<Vec<Address>> {
    let source = text(lbp, "src/LBP.sol")?;
    let getter = "function balanceOf(address account) public view override returns (uint256) {\n        uint256 raw = super.balanceOf(account);\n        if (\n            account == address(this) || account == pair || account == DEAD\n                || account == address(burnVault) || account == address(refVault)\n                || account == address(polVault) || account == address(fomoVault)\n                || account == address(hashrate)\n        ) {\n            return raw;\n        }\n        return raw + hashrate.pendingRewards(account);\n    }";
    ensure!(
        source.contains(getter) && source.contains("address public constant DEAD = 0x000000000000000000000000000000000000dEaD;"),
        "selected LBP getter branches"
    );
    ensure!(
        immutable_address(lbp, "17607")? == address(CAPTURES[1].1)? && immutable_address(hlbp, "9180")? == address(CAPTURES[0].1)?,
        "LBP/hLBP dependency identity"
    );
    ensure!(
        immutable_address(lbp, "17591")? == immutable_address(hlbp, "9184")? && immutable_address(lbp, "17597")? == immutable_address(hlbp, "9187")?,
        "shared pair/refVault identity"
    );
    let mut exemptions = vec![address(CAPTURES[0].1)?, address("0x000000000000000000000000000000000000dead")?];
    for id in ["17591", "17594", "17597", "17600", "17603", "17607"] {
        exemptions.push(immutable_address(lbp, id)?);
    }
    ensure!(
        exemptions.iter().copied().collect::<BTreeSet<_>>().len() == 8 && !exemptions.contains(&[0; 20]),
        "exact exemption set"
    );
    Ok(exemptions)
}
