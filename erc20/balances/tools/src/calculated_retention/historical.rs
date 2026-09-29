//! Immutable historical storage checkpoints and separate getter expectations.
//! The parent capture has no parent-of-parent hash or Extended producer version;
//! neither is inferred from a later block. Getter amounts never initialize facts.
use super::{address, binding, word, Address, At, Checkpoint, Fact, Ledger, Limits, Origin, Slot, Word};
use anyhow::{ensure, Context, Result};
use binding::{Binding, Model};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const PARENT_NUMBER: u64 = 122288005;
pub const LAST_NUMBER: u64 = 122289029;
pub const PARENT_TIMESTAMP: u64 = 1789591601;
pub const PARENT_HASH: &str = "0xdf9d02ee1c9e345bfcdb971e488a20f7f51dc03d3d8e2e7bbc8fc9f0bb0814cf";

pub struct Original {
    pub label: &'static str,
    pub path: &'static str,
    pub raw: &'static [u8],
    pub sha256: &'static str,
}

pub const ORIGINALS: [Original; 9] = [
    Original {
        label: "lbp-manifest",
        path: "erc20/balances/tests/fixtures/lbp-rewards/manifest.json",
        raw: include_bytes!("../../../tests/fixtures/lbp-rewards/manifest.json"),
        sha256: "848f7f9038c520e7856ce453b4300cbae5659ae08e5fb2dc1fd5bc3f8e03ccbf",
    },
    Original {
        label: "lbp-checkpoint",
        path: "erc20/balances/tests/fixtures/lbp-rewards/checkpoint.json",
        raw: include_bytes!("../../../tests/fixtures/lbp-rewards/checkpoint.json"),
        sha256: "7376734069834de824562c7b0fb18855e2c51292152c7b636ee5e266d0310c86",
    },
    Original {
        label: "lbp-replay",
        path: "erc20/balances/tests/fixtures/lbp-rewards/replay.json",
        raw: include_bytes!("../../../tests/fixtures/lbp-rewards/replay.json"),
        sha256: "2a81307de25a5301fa026ba9b170c3f1d041e1e475476d626ed4f97aca752bc4",
    },
    Original {
        label: "lbp-checks",
        path: "erc20/balances/tests/fixtures/lbp-rewards/checks.json",
        raw: include_bytes!("../../../tests/fixtures/lbp-rewards/checks.json"),
        sha256: "b58ee5bdb5cd669e120383cbb52d9b7a4d6d575c0e3f6f3e357cf451be7ad887",
    },
    Original {
        label: "lbp-controls",
        path: "erc20/balances/tests/fixtures/lbp-rewards/controls.json",
        raw: include_bytes!("../../../tests/fixtures/lbp-rewards/controls.json"),
        sha256: "7d81c13da4a83fdb24de1ca7af01d9a91df329f5d698c97bddc2d4b01f1b1dc7",
    },
    Original {
        label: "babydoge",
        path: "erc20/balances/tools/tests/fixtures/reflection/BabyDoge.json",
        raw: include_bytes!("../../tests/fixtures/reflection/BabyDoge.json"),
        sha256: "5e248c83e7c9d82c54d51a9742c47aab62182d539075ad97a492bd0e774da706",
    },
    Original {
        label: "tenset",
        path: "erc20/balances/tools/tests/fixtures/reflection/10SET.json",
        raw: include_bytes!("../../tests/fixtures/reflection/10SET.json"),
        sha256: "14a83963515efea0bdc6479a2f220c219710e280f5a7b96176f4be472293f8e5",
    },
    Original {
        label: "reflection-controls",
        path: "erc20/balances/tools/tests/fixtures/reflection/controls.json",
        raw: include_bytes!("../../tests/fixtures/reflection/controls.json"),
        sha256: "1f9e23c3c1be2ae6e33ee5e5c3d554df96cbfa604b3b1522b3e24d8efd6f61c4",
    },
    Original {
        label: "tenset-passive",
        path: "erc20/balances/tools/tests/fixtures/reflection/10SET-passive.json",
        raw: include_bytes!("../../tests/fixtures/reflection/10SET-passive.json"),
        sha256: "46ccbdf245b5067197ba39e780a16962c382ef73a03b81c29b48b94bc6ad918c",
    },
];

fn descriptor(label: &str) -> Result<&'static Original> {
    ORIGINALS.iter().find(|v| v.label == label).context("unknown historical original")
}

/// Verify complete original bytes before JSON parsing, including duplicate-key
/// mutations that a generic JSON object decoder might otherwise collapse.
pub fn verify_original(label: &str, raw: &[u8]) -> Result<Value> {
    ensure!(binding::sha(raw) == descriptor(label)?.sha256, "historical {label} raw hash mismatch");
    Ok(serde_json::from_slice(raw)?)
}

pub fn original(label: &str) -> Result<Value> {
    verify_original(label, descriptor(label)?.raw)
}

fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().map(Vec::as_slice).context("historical array")
}
fn number(v: &Value) -> Result<u64> {
    v.as_u64().context("historical unsigned number")
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().context("historical string")
}
fn hex_word(v: &Value) -> Result<Word> {
    hex::decode(text(v)?.strip_prefix("0x").context("historical word prefix")?)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("historical word width"))
}
fn decimal_word(v: &Value) -> Result<Word> {
    Ok(word(crate::data::uint(v)?))
}
fn accounts(rows: &[Value], field: Option<&str>) -> Result<Vec<Address>> {
    let mut result = BTreeSet::new();
    for row in rows {
        let h = address(text(field.map_or(row, |key| &row[key]))?)?;
        ensure!(result.insert(h), "duplicate historical holder");
    }
    Ok(result.into_iter().collect())
}

/// The selected parent identity comes from the pinned LBP replay; absent fields
/// remain absent until the collector accepts the first actual Extended block.
pub fn parent() -> Result<At> {
    let replay = original("lbp-replay")?;
    let p = &replay["parent"];
    ensure!(
        number(&p["number"])? == PARENT_NUMBER && number(&p["timestamp"])? == PARENT_TIMESTAMP && p["hash"] == PARENT_HASH,
        "historical parent changed"
    );
    ensure!(p.as_object().context("parent object")?.len() == 3, "unexpected parent metadata");
    Ok(At {
        number: PARENT_NUMBER,
        hash: hex_word(&p["hash"])?,
        parent_hash: None,
        timestamp: PARENT_TIMESTAMP,
        producer_version: None,
    })
}

fn clocks(replay: &Value) -> Result<BTreeMap<u64, (Word, u64)>> {
    let p = parent()?;
    let mut clocks = BTreeMap::from([(p.number, (p.hash, p.timestamp))]);
    let mut previous = (p.number, p.hash, p.timestamp);
    let blocks = array(&replay["blocks"])?;
    ensure!(blocks.len() == 1024, "historical window length changed");
    let mut updates = 0;
    for block in blocks {
        let n = number(&block["number"])?;
        let hash = hex_word(&block["hash"])?;
        let timestamp = number(&block["timestamp"])?;
        ensure!(
            n == previous.0 + 1 && hex_word(&block["parent_hash"])? == previous.1 && timestamp >= previous.2,
            "historical clock discontinuity"
        );
        ensure!(binding::is_sha(text(&block["source_sha256"])?), "historical PB digest missing");
        updates += array(&block["updates"])?.len();
        ensure!(clocks.insert(n, (hash, timestamp)).is_none(), "duplicate historical clock");
        previous = (n, hash, timestamp);
    }
    ensure!(previous.0 == LAST_NUMBER && updates == 110, "historical replay scope changed");
    Ok(clocks)
}

fn reflection_binding(v: &Value, binding: &Binding) -> Result<()> {
    ensure!(address(text(&v["contract"])?)? == binding.token(), "reflection contract differs");
    ensure!(
        v["source_sha256"] == binding.codes[0].capture_sha256 && hex_word(&v["runtime_hash"])? == binding.codes[0].runtime_hash,
        "reflection capture/runtime differs"
    );
    ensure!(
        v["layout"] == serde_json::to_value(binding.reflection_layout().context("reflection model")?)?,
        "reflection formula layout differs"
    );
    let index = if binding.model == Model::BabyDoge { 2 } else { 3 };
    let capture: Value = serde_json::from_slice(binding::CAPTURES[index].3)?;
    ensure!(v["storage_layout"] == capture["storageLayout"], "reflection captured layout differs");
    Ok(())
}

fn storage(v: &Value) -> Result<BTreeMap<Word, Word>> {
    let mut result = BTreeMap::new();
    for (key, value) in v.as_object().context("historical storage object")? {
        ensure!(
            result.insert(hex_word(&Value::String(key.clone()))?, hex_word(value)?).is_none(),
            "duplicate canonical historical storage key"
        );
    }
    Ok(result)
}

fn verified() -> Result<BTreeMap<&'static str, Value>> {
    let values = ORIGINALS
        .iter()
        .map(|v| Ok((v.label, verify_original(v.label, v.raw)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let manifest = &values["lbp-manifest"];
    ensure!(
        manifest["chain_id"] == 56 && manifest["schema"] == 1 && manifest["parent_hash"] == PARENT_HASH,
        "LBP manifest identity differs"
    );
    for (field, label) in [
        ("checkpoint_sha256", "lbp-checkpoint"),
        ("checks_sha256", "lbp-checks"),
        ("replay_sha256", "lbp-replay"),
        ("controls_sha256", "lbp-controls"),
    ] {
        ensure!(manifest[field] == descriptor(label)?.sha256, "LBP manifest {field} crosslink differs");
    }
    for field in ["checkpoint_sha256", "checks_sha256"] {
        ensure!(manifest["source_report"][field] == manifest[field], "LBP source report crosslink differs");
    }
    ensure!(array(&values["lbp-controls"])?.len() == 34, "LBP control scope differs");
    let controls = &values["reflection-controls"];
    ensure!(
        array(&controls["tokens"])?
            .iter()
            .map(|t| array(&t["cases"]).map(|c| c.len()))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .sum::<usize>()
            == 48,
        "reflection control scope differs"
    );
    let passive = &values["tenset-passive"];
    let ten = Binding::historical(Model::TenSet, 1, PARENT_NUMBER)?;
    ensure!(
        address(text(&passive["contract"])?)? == ten.token() && passive["layout"] == serde_json::to_value(ten.reflection_layout())?,
        "passive model binding differs"
    );
    ensure!(
        array(&passive["cases"])?.len() == 12 && array(&passive["independent_rpc_checks"])?.len() == 24,
        "passive control scope differs"
    );
    Ok(values)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub model: Model,
    pub number: u64,
    pub hash: Word,
    pub holder: Address,
    pub amount: Word,
    pub pending: Option<Word>,
}

/// Independent getter expectations, never inputs to `checkpoints` or `Fact`.
pub fn observations() -> Result<Vec<Observation>> {
    let originals = verified()?;
    let clocks = clocks(&originals["lbp-replay"])?;
    let lbp_holders = accounts(array(&originals["lbp-replay"]["holders"])?, None)?;
    ensure!(lbp_holders.len() == 33, "LBP holder scope differs");
    let mut result = Vec::new();
    let mut observed = BTreeMap::<u64, BTreeSet<Address>>::new();
    for row in array(&originals["lbp-checks"])? {
        let n = number(&row["block"])?;
        let hash = hex_word(&row["hash"])?;
        let holder = address(text(&row["holder"])?)?;
        ensure!(clocks.get(&n).map(|c| c.0) == Some(hash), "LBP getter clock differs");
        ensure!(observed.entry(n).or_default().insert(holder), "duplicate LBP getter expectation");
        ensure!(
            lbp_holders.contains(&holder) && row["match"] == true && row["rpc"] == row["computed"] && row["rpc_pending"] == row["computed_pending"],
            "LBP getter evidence differs"
        );
        result.push(Observation {
            model: Model::Lbp,
            number: n,
            hash,
            holder,
            amount: decimal_word(&row["rpc"])?,
            pending: Some(decimal_word(&row["rpc_pending"])?),
        });
    }
    ensure!(
        result.len() == 594 && observed.len() == 18 && observed.values().all(|set| set.iter().copied().collect::<Vec<_>>() == lbp_holders),
        "LBP getter cohort incomplete"
    );
    for (label, model, count, snapshots) in [("babydoge", Model::BabyDoge, 19, 11), ("tenset", Model::TenSet, 12, 10)] {
        let v = &originals[label];
        let binding = Binding::historical(model, 1, PARENT_NUMBER)?;
        reflection_binding(v, &binding)?;
        let snaps = array(&v["snapshots"])?;
        ensure!(snaps.len() == snapshots, "reflection snapshot scope differs");
        let cohort = accounts(array(&snaps[0]["observations"])?, Some("holder"))?;
        ensure!(cohort.len() == count, "reflection holder scope differs");
        let mut previous = None;
        for snap in snaps {
            let n = number(&snap["block"])?;
            let hash = hex_word(&snap["hash"])?;
            ensure!(
                previous.is_none_or(|p| p < n) && clocks.get(&n).map(|c| c.0) == Some(hash),
                "reflection snapshot clock differs"
            );
            previous = Some(n);
            ensure!(accounts(array(&snap["observations"])?, Some("holder"))? == cohort, "reflection cohort changed");
            storage(&snap["storage"])?;
            for row in array(&snap["observations"])? {
                result.push(Observation {
                    model,
                    number: n,
                    hash,
                    holder: address(text(&row["holder"])?)?,
                    amount: decimal_word(&row["balance_of"])?,
                    pending: None,
                });
            }
        }
    }
    ensure!(result.len() == 923, "combined expectation scope differs");
    Ok(result)
}

fn checkpoint(binding: Binding, holders: Vec<Address>, raw: BTreeMap<Slot, Word>, evidence: String) -> Result<Checkpoint> {
    let at = parent()?;
    let words = raw
        .into_iter()
        .map(|(slot, word)| Fact {
            slot,
            word,
            at: at.clone(),
            epoch: binding.epoch,
            origin: Origin::Checkpoint { evidence: evidence.clone() },
        })
        .collect();
    let checkpoint = Checkpoint {
        binding,
        at,
        holders,
        words,
        evidence,
    };
    Ledger::from_checkpoint(checkpoint.clone(), Limits::default())?;
    Ok(checkpoint)
}

pub fn checkpoints() -> Result<Vec<Checkpoint>> {
    let originals = verified()?;
    clocks(&originals["lbp-replay"])?;
    // Validate all independent observation identities, without passing their
    // amounts into the raw checkpoint construction below.
    observations()?;
    let lbp = Binding::historical(Model::Lbp, 1, PARENT_NUMBER)?;
    let holders = accounts(array(&originals["lbp-replay"]["holders"])?, None)?;
    ensure!(holders.len() == 33, "LBP checkpoint cohort differs");
    let mut raw = BTreeMap::new();
    for row in array(&originals["lbp-checkpoint"])? {
        ensure!(row["hash"] == PARENT_HASH, "LBP checkpoint row hash differs");
        let slot = Slot {
            contract: address(text(&row["contract"])?)?,
            key: hex_word(&row["key"])?,
        };
        ensure!(
            lbp.protected(slot.contract) && raw.insert(slot, decimal_word(&row["word"])?).is_none(),
            "duplicate or foreign LBP checkpoint fact"
        );
    }
    ensure!(raw.len() == 171, "LBP checkpoint word count differs");
    let mut result = vec![checkpoint(lbp, holders, raw, descriptor("lbp-checkpoint")?.sha256.into())?];
    for (label, model, count, words) in [("babydoge", Model::BabyDoge, 19, 102), ("tenset", Model::TenSet, 12, 43)] {
        let v = &originals[label];
        let binding = Binding::historical(model, 1, PARENT_NUMBER)?;
        reflection_binding(v, &binding)?;
        let snap = array(&v["snapshots"])?.first().context("initial reflection snapshot")?;
        ensure!(
            snap["block"] == PARENT_NUMBER && snap["hash"] == PARENT_HASH,
            "reflection checkpoint clock differs"
        );
        let holders = accounts(array(&snap["observations"])?, Some("holder"))?;
        let raw = storage(&snap["storage"])?;
        ensure!(holders.len() == count && raw.len() == words, "reflection initial scope differs");
        let raw = raw
            .into_iter()
            .map(|(key, word)| {
                (
                    Slot {
                        contract: binding.token(),
                        key,
                    },
                    word,
                )
            })
            .collect();
        result.push(checkpoint(binding, holders, raw, descriptor(label)?.sha256.into())?);
    }
    Ok(result)
}
