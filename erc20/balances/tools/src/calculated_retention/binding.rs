//! Closed historical descriptors, not a declaration that present deployments are
//! qualified. Exact captures preserve their original source/verification labels.
use super::{address, Address, Word};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Model {
    Lbp,
    BabyDoge,
    TenSet,
}
impl Model {
    pub const ALL: [Self; 3] = [Self::Lbp, Self::BabyDoge, Self::TenSet];
    pub fn label(self) -> &'static str {
        match self {
            Self::Lbp => "LBP",
            Self::BabyDoge => "BabyDoge",
            Self::TenSet => "10SET",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeIdentity {
    pub contract: Address,
    pub runtime_hash: Word,
    pub capture_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: u32,
    pub model_revision: u32,
    pub persistence_revision: u32,
    pub chain_id: u64,
    pub model: Model,
    pub epoch: u32,
    pub activation_block: u64,
    pub codes: Vec<CodeIdentity>,
    pub exemptions: Vec<Address>,
    pub oracle_sha256: Vec<String>,
}
pub const CAPTURES: [(&str, &str, &str, &[u8]); 4] = [
    (
        "lbp",
        "0x88886f0fd371dff856291badced45922bc888888",
        "def766d9a5568fd07f95ea88cb8dfade485d452addd0a2698c495032b9cff2dd",
        include_bytes!("../../tests/fixtures/calculated-retention/lbp-source.json"),
    ),
    (
        "hlbp",
        "0x5e3cbc82d020be91a989eb747934104e9ab585fe",
        "5b61fc47bb9852f0a362309d06cd638c3a6c3540d1c6a2ad5cb778df736d0d8b",
        include_bytes!("../../tests/fixtures/calculated-retention/hlbp-source.json"),
    ),
    (
        "babydoge",
        "0xc748673057861a797275cd8a068abb95a902e8de",
        "a83222287f471061fb345c52e4e3565591fb1e7253a554dab65fccf3ea4356dd",
        include_bytes!("../../tests/fixtures/calculated-retention/babydoge-source.json"),
    ),
    (
        "tenset",
        "0x1f64fdad335ed784898effb5ce22d54d8f432523",
        "083a44ab01fd109dad4e7a4b7c5a4061b963454a86a35d9f0be439a045c8d87d",
        include_bytes!("../../tests/fixtures/calculated-retention/tenset-source.json"),
    ),
];
pub const LBP_MANIFEST: &[u8] = include_bytes!("../../../tests/fixtures/lbp-rewards/manifest.json");
pub const REFLECTION: [&[u8]; 2] = [
    include_bytes!("../../tests/fixtures/reflection/BabyDoge.json"),
    include_bytes!("../../tests/fixtures/reflection/10SET.json"),
];
pub fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub fn is_sha(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn bytes(s: &Value) -> Result<Vec<u8>> {
    hex::decode(s.as_str().context("hex string")?.strip_prefix("0x").context("hex prefix")?).map_err(Into::into)
}
pub fn verify_capture(raw: &[u8], index: usize) -> Result<CodeIdentity> {
    let (_, contract, pin, _) = CAPTURES.get(index).context("capture index")?;
    ensure!(sha(raw) == *pin, "capture raw hash mismatch");
    let v: Value = serde_json::from_slice(raw)?;
    super::source::verify(&v, index)?;
    ensure!(
        address(v["address"].as_str().context("capture address")?)? == address(contract)? && (v["chainId"] == 56 || v["chainId"] == "56"),
        "capture network/address mismatch"
    );
    let runtime = bytes(&v["runtimeBytecode"]["onchainBytecode"])?;
    ensure!(!runtime.is_empty(), "missing captured runtime");
    // Whole capture SHA pins all sources, compiler settings/layout/metadata and
    // declared transformations; no byte stripping or new recompilation is claimed.
    ensure!(
        v["sources"].as_object().is_some_and(|s| !s.is_empty()) && v["storageLayout"]["storage"].is_array(),
        "missing complete source/layout capture"
    );
    Ok(CodeIdentity {
        contract: address(contract)?,
        runtime_hash: erc20_balances::hash(&runtime),
        capture_sha256: (*pin).into(),
    })
}
fn templates() -> Result<&'static Vec<Binding>> {
    static VALUE: OnceLock<std::result::Result<Vec<Binding>, String>> = OnceLock::new();
    VALUE
        .get_or_init(|| {
            (|| -> Result<Vec<Binding>> {
                let codes = CAPTURES
                    .iter()
                    .enumerate()
                    .map(|(i, (_, _, _, raw))| verify_capture(raw, i))
                    .collect::<Result<Vec<_>>>()?;
                ensure!(
                    sha(LBP_MANIFEST) == "848f7f9038c520e7856ce453b4300cbae5659ae08e5fb2dc1fd5bc3f8e03ccbf",
                    "LBP manifest changed"
                );
                let manifest: Value = serde_json::from_slice(LBP_MANIFEST)?;
                let exemptions = manifest["exemptions"]
                    .as_array()
                    .context("exemptions")?
                    .iter()
                    .map(|v| address(v.as_str().context("exempt address")?))
                    .collect::<Result<Vec<_>>>()?;
                ensure!(exemptions.len() == 8, "exemption scope changed");
                let lbp: Value = serde_json::from_slice(CAPTURES[0].3)?;
                let hlbp: Value = serde_json::from_slice(CAPTURES[1].3)?;
                ensure!(exemptions == super::source::lbp_exemptions(&lbp, &hlbp)?, "manifest exemption branch differs");
                let mut result = vec![Binding {
                    schema: 1,
                    model_revision: 1,
                    persistence_revision: 1,
                    chain_id: 56,
                    model: Model::Lbp,
                    epoch: 1,
                    activation_block: 122288005,
                    codes: codes[..2].to_vec(),
                    exemptions,
                    oracle_sha256: vec![sha(LBP_MANIFEST)],
                }];
                for (index, model, pin) in [
                    (0, Model::BabyDoge, "5e248c83e7c9d82c54d51a9742c47aab62182d539075ad97a492bd0e774da706"),
                    (1, Model::TenSet, "14a83963515efea0bdc6479a2f220c219710e280f5a7b96176f4be472293f8e5"),
                ] {
                    let raw = REFLECTION[index];
                    ensure!(sha(raw) == pin, "reflection oracle changed");
                    let oracle: Value = serde_json::from_slice(raw)?;
                    ensure!(
                        oracle["source_sha256"] == codes[index + 2].capture_sha256
                            && oracle["runtime_hash"] == format!("0x{}", hex::encode(codes[index + 2].runtime_hash)),
                        "reflection source/runtime binding changed"
                    );
                    result.push(Binding {
                        schema: 1,
                        model_revision: 1,
                        persistence_revision: 1,
                        chain_id: 56,
                        model,
                        epoch: 1,
                        activation_block: 122288005,
                        codes: vec![codes[index + 2].clone()],
                        exemptions: vec![],
                        oracle_sha256: vec![pin.into()],
                    });
                }
                Ok(result)
            })()
            .map_err(|e| format!("{e:#}"))
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}
impl Binding {
    pub fn historical(model: Model, epoch: u32, activation_block: u64) -> Result<Self> {
        ensure!(epoch > 0 && activation_block >= 122288005, "invalid historical epoch/activation");
        let mut result = templates()?.iter().find(|b| b.model == model).unwrap().clone();
        result.epoch = epoch;
        result.activation_block = activation_block;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            *self == Self::historical(self.model, self.epoch, self.activation_block)?,
            "unsupported or changed model binding"
        );
        Ok(())
    }
    pub fn digest(&self) -> Result<String> {
        Ok(sha(&serde_json::to_vec(self)?))
    }
    pub fn token(&self) -> Address {
        self.codes[0].contract
    }
    pub fn dependency(&self) -> Option<Address> {
        self.codes.get(1).map(|c| c.contract)
    }
    pub fn protected(&self, address: Address) -> bool {
        self.codes.iter().any(|c| c.contract == address)
    }
    pub fn code_hash(&self, address: Address) -> Option<Word> {
        self.codes.iter().find(|c| c.contract == address).map(|c| c.runtime_hash)
    }
    pub fn reflection_layout(&self) -> Option<crate::reflection::fixture::Layout> {
        use crate::reflection::fixture::Layout;
        match self.model {
            Model::Lbp => None,
            Model::BabyDoge => Some(Layout {
                reflections: 3,
                tokens: 4,
                excluded_flag: 7,
                excluded_array: 8,
                total_reflections: 10,
                total_tokens: 9,
            }),
            Model::TenSet => Some(Layout {
                reflections: 1,
                tokens: 2,
                excluded_flag: 4,
                excluded_array: 5,
                total_reflections: 7,
                total_tokens: 6,
            }),
        }
    }
}
