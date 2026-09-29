//! Local-only imports of exact raw historical words and independent override
//! branches. Decoded fixture states/getter expectations never initialize facts.
use super::{
    address,
    binding::{self, Binding},
    decode, At, Checkpoint, Fact, Ledger, Limits, Metric, Origin, Slot,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
pub const PARENT: u64 = 122288005;
pub const FINAL: u64 = 122289029;
pub const CONTROL_HOLDER: &str = "0x2aafce28b5552e216811fb5a1cb015178a29b461";
fn rows<'a>(v: &'a Value, label: &str) -> Result<&'a [Value]> {
    v.as_array().map(Vec::as_slice).with_context(|| format!("{label} array"))
}
pub fn raw_words(v: &Value) -> Result<BTreeMap<Slot, super::Word>> {
    let mut words = BTreeMap::new();
    for row in rows(v, "raw storage")? {
        ensure!(row.as_object().is_some_and(|o| o.len() == 3), "raw row schema");
        let slot = Slot {
            contract: address(row["account"].as_str().context("raw account")?)?,
            key: binding::hexword(&row["key"])?,
        };
        ensure!(binding::protected(slot.contract) && slot.contract != binding::helper(), "raw storage contract");
        ensure!(words.insert(slot, binding::hexword(&row["value"])?).is_none(), "duplicate raw word");
    }
    Ok(words)
}
pub fn captured_words(number: u64) -> Result<BTreeMap<Slot, super::Word>> {
    let file = match number {
        PARENT => "parent-storage.json",
        FINAL => "final-storage.json",
        _ => anyhow::bail!("uncaptured checkpoint"),
    };
    let words = raw_words(&binding::original(file)?)?;
    ensure!(words.len() == 1705, "raw checkpoint count");
    Ok(words)
}
pub fn checkpoint(number: u64) -> Result<Checkpoint> {
    let report = binding::original("historical.json")?;
    let boundary = rows(&report["boundaries"], "boundaries")?
        .iter()
        .find(|b| b["block"] == number)
        .context("uncaptured boundary")?;
    let at = At {
        number,
        hash: binding::hexword(&boundary["hash"])?,
        parent_hash: None,
        timestamp: boundary["timestamp"].as_str().context("timestamp")?.parse()?,
        producer_version: None,
    };
    let evidence = boundary["storage_sha256"].as_str().context("storage hash")?.to_string();
    let words = captured_words(number)?;
    let mut holders = BTreeSet::new();
    for row in rows(&report["snapshots"], "snapshots")?.iter().filter(|r| r["block"] == number) {
        ensure!(
            binding::hexword(&row["hash"])? == at.hash && holders.insert(address(row["holder"].as_str().context("holder")?)?),
            "snapshot identity/duplicate"
        );
    }
    ensure!(holders.len() == 12, "finite holder scope");
    // Key NAMES only: final capture cannot populate a parent's fact or origin.
    let universe = captured_words(PARENT)?
        .into_keys()
        .chain(captured_words(FINAL)?.into_keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let facts = words
        .into_iter()
        .map(|(slot, word)| Fact {
            slot,
            word,
            at: at.clone(),
            epoch: 1,
            origin: Origin::Checkpoint { evidence: evidence.clone() },
        })
        .collect();
    Ok(Checkpoint {
        binding: Binding::historical(1, number)?,
        at,
        holders: holders.into_iter().collect(),
        universe,
        facts,
        evidence,
    })
}
pub fn override_checkpoint(index: usize) -> Result<Checkpoint> {
    let report = binding::original("overrides.json")?;
    let case = rows(&report["cases"], "cases")?.get(index).context("override index")?;
    let mut cp = checkpoint(PARENT)?;
    ensure!(
        report["block"] == PARENT && binding::hexword(&report["hash"])? == cp.at.hash,
        "override boundary"
    );
    let mut words: BTreeMap<_, _> = cp.facts.into_iter().map(|f| (f.slot, f.word)).collect();
    let overrides = case["overrides"].as_object().context("raw overrides")?;
    ensure!(overrides.len() == 2, "override account scope");
    for (a, entry) in overrides {
        let a = address(a)?;
        ensure!(
            [binding::token(), binding::pool()].contains(&a) && entry.as_object().is_some_and(|o| o.len() == 1),
            "override schema"
        );
        for (key, val) in entry["stateDiff"].as_object().context("stateDiff")? {
            words.insert(
                Slot {
                    contract: a,
                    key: binding::hexword(&json!(key))?,
                },
                binding::hexword(val)?,
            );
        }
    }
    cp.holders = vec![address(CONTROL_HOLDER)?];
    cp.evidence = format!(
        "synthetic-state-override:{}:{index}:{}",
        binding::ORIGINALS[6].sha256,
        case["name"].as_str().context("case name")?
    );
    cp.universe = words.keys().cloned().collect();
    cp.facts = words
        .into_iter()
        .map(|(slot, word)| Fact {
            slot,
            word,
            at: cp.at.clone(),
            epoch: 1,
            origin: Origin::Checkpoint { evidence: cp.evidence.clone() },
        })
        .collect();
    Ok(cp)
}
pub fn check_captured() -> Result<Value> {
    let report = binding::original("historical.json")?;
    let mut comparisons = Vec::new();
    for n in [PARENT, FINAL] {
        let ledger = Ledger::from_checkpoint(checkpoint(n)?, Limits::default())?;
        for expected in rows(&report["snapshots"], "snapshots")?.iter().filter(|v| v["block"] == n) {
            let h = address(expected["holder"].as_str().context("holder")?)?;
            let result = ledger.evaluate(h)?;
            ensure!(
                result.pending
                    == Metric::Known(decode::Reward {
                        pending_reward: expected["expected_reward"].as_str().context("reward expectation")?.into(),
                        stopping_hour: expected["expected_stopping_hour"].as_str().context("stop expectation")?.into()
                    })
                    && result.observable == Metric::Known(expected["expected_balance"].as_str().context("balance expectation")?.into()),
                "captured getter differs: {n} {}: {result:?}",
                hex::encode(h)
            );
            comparisons.push(json!({"number":n,"holder":h,"evaluation":result}));
        }
    }
    let original = binding::original("overrides.json")?;
    let mut controls = Vec::new();
    let mut reverts = 0;
    for (index, case) in rows(&original["cases"], "cases")?.iter().enumerate() {
        let l = Ledger::from_checkpoint(override_checkpoint(index)?, Limits::default())?;
        let e = l.evaluate(address(CONTROL_HOLDER)?)?;
        for check in rows(&case["checks"], "checks")? {
            let observed: std::result::Result<Vec<String>, String> = if check["getter"] == "reward" {
                match &e.pending {
                    Metric::Known(r) => Ok(vec![r.pending_reward.clone(), r.stopping_hour.clone()]),
                    Metric::ModelRefusal { reason } => Err(reason.clone()),
                    other => anyhow::bail!("unexpected reward outcome {other:?}"),
                }
            } else {
                match &e.observable {
                    Metric::Known(v) => Ok(vec![v.clone()]),
                    Metric::ModelRefusal { reason } => Err(reason.clone()),
                    other => anyhow::bail!("unexpected balance outcome {other:?}"),
                }
            };
            match observed {
                Ok(v) => ensure!(json!(v) == check["rpc"], "override result differs: {}", case["name"]),
                Err(reason) => {
                    reverts += 1;
                    let data = check["response"]["error"]["data"].as_str().context("captured revert data")?;
                    ensure!(
                        check["rpc"].is_null()
                            && check["model_error"] == reason
                            && if reason == "INSUFFICIENT_LIQUIDITY" {
                                data == format!("0x08c379a0{:064x}{:064x}{:0<64}", 32, 22, hex::encode("INSUFFICIENT_LIQUIDITY"))
                            } else {
                                data == format!("0x4e487b71{:064x}", 17)
                            },
                        "override refusal differs: {}",
                        case["name"]
                    );
                }
            }
        }
        controls.push(json!({"name":case["name"],"evaluation":e}));
    }
    ensure!(comparisons.len() == 24 && controls.len() == 30 && reverts == 17, "captured comparison count");
    Ok(
        json!({"checkpoint_comparisons":comparisons,"override_comparisons":controls,"historical_getters":48,"override_getters":60,"recognized_override_reverts":reverts}),
    )
}
