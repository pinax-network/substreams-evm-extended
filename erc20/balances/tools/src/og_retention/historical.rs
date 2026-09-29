//! Original OG storage inputs and explicitly counterfactual state/time controls.
//! Getter expectations, recursive error payloads and gas observations remain
//! separate evidence; none can initialize or repair a raw fact.
use super::{address, binding, At, Binding, Checkpoint, Fact, Ledger, Limits, Origin, Slot};
use crate::calculated_retention::binding::sha;
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const HISTORICAL_WORDS: [usize; 5] = [153, 114, 633, 114, 114];
pub const CONTROL_SCOPE: &str = "synthetic raw state/time override at a captured identity; not a historical continuation";

fn list(v: &Value) -> Result<&[Value]> {
    v.as_array().map(Vec::as_slice).context("OG evidence array")
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().context("OG evidence string")
}
fn number(v: &Value) -> Result<u64> {
    v.as_u64().context("OG evidence number")
}
fn timestamp(v: &Value) -> Result<u64> {
    let s = text(v)?.strip_prefix("0x").context("OG timestamp prefix")?;
    ensure!(!s.is_empty() && s.len() <= 16, "OG timestamp width");
    Ok(u64::from_str_radix(s, 16)?)
}
fn descriptor(file: &str) -> Result<&'static binding::Original> {
    binding::ORIGINALS.iter().find(|o| o.file == file).context("unknown OG evidence file")
}

fn captured_identity(v: &Value) -> Result<Value> {
    let historical = binding::original("historical.json")?;
    let case = number(&v["case"])?;
    let matching: Vec<_> = list(&historical)?.iter().filter(|r| r["case"] == case).collect();
    ensure!(matching.len() == 1, "unknown/duplicate OG historical case");
    let captured = matching[0];
    ensure!(v["contract"] == binding::TOKEN, "OG checkpoint token differs");
    ensure!(
        number(&v["block"])? == number(&captured["block"])?
            && binding::hexword(&v["hash"])? == binding::hexword(&captured["hash"])?
            && address(text(&v["holder"])?)? == address(text(&captured["holder"])?)?,
        "OG checkpoint historical identity differs"
    );
    Ok(captured.clone())
}

fn runtimes(v: &Value) -> Result<()> {
    let mut actual = BTreeMap::new();
    for row in list(&v["runtime_bindings"])? {
        let contract = address(text(&row["address"])?)?;
        ensure!(
            actual.insert(contract, binding::hexword(&row["runtime_hash"])?).is_none(),
            "duplicate OG runtime address"
        );
    }
    ensure!(actual == binding::codes().into_iter().collect(), "OG five-runtime identity differs");
    Ok(())
}

fn raw_rows(v: &Value) -> Result<BTreeMap<Slot, super::Word>> {
    let mut raw = BTreeMap::new();
    for row in list(v)? {
        let slot = Slot {
            contract: address(text(&row["contract"])?)?,
            key: binding::hexword(&row["key"])?,
        };
        ensure!(binding::retainable(&slot), "OG raw word outside retained input contracts");
        ensure!(raw.insert(slot, binding::hexword(&row["value"])?).is_none(), "duplicate canonical OG raw word");
    }
    Ok(raw)
}

fn import(v: &Value, evidence: &str, counterfactual: bool) -> Result<Checkpoint> {
    ensure!(!evidence.trim().is_empty(), "OG evidence label required");
    let captured = captured_identity(v)?;
    if !counterfactual {
        ensure!(
            timestamp(&v["timestamp"])? == timestamp(&captured["timestamp"])?,
            "historical timestamp differs; use an explicit control override"
        );
    }
    runtimes(v)?;
    let at = At {
        number: number(&v["block"])?,
        hash: binding::hexword(&v["hash"])?,
        parent_hash: None,
        timestamp: timestamp(&v["timestamp"])?,
        producer_version: None,
    };
    let binding = Binding::historical(1, at.number)?;
    let raw = raw_rows(&v["raw_state"])?;
    let mut holders = vec![address(text(&v["holder"])?)?, binding::pool_a()];
    holders.sort();
    ensure!(holders[0] != holders[1], "historical holder unexpectedly equals pool A");
    let universe = raw.keys().cloned().collect();
    let facts = raw
        .into_iter()
        .map(|(slot, word)| Fact {
            slot,
            word,
            at: at.clone(),
            epoch: binding.epoch,
            origin: Origin::Checkpoint { evidence: evidence.into() },
        })
        .collect();
    let cp = Checkpoint {
        binding,
        at,
        holders,
        universe,
        facts,
        evidence: evidence.into(),
    };
    Ledger::from_checkpoint(cp.clone(), Limits::default())?;
    Ok(cp)
}

/// Structural raw importer. Exact historical identity and five runtime hashes
/// are required, but missing raw words remain missing for explicit Unknown tests.
/// `checkpoints` additionally binds the original complete bytes and word counts.
pub fn checkpoint_from_raw(v: &Value, evidence: &str) -> Result<Checkpoint> {
    import(v, evidence, false)
}

pub fn checkpoints() -> Result<Vec<Checkpoint>> {
    let raw = binding::original("historical.json")?;
    let rows = list(&raw)?;
    ensure!(rows.len() == 5, "OG historical scope differs");
    let mut result = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        ensure!(number(&row["case"])? == i as u64 + 1, "OG historical case order differs");
        let cp = checkpoint_from_raw(row, descriptor("historical.json")?.sha256)?;
        ensure!(cp.facts.len() == HISTORICAL_WORDS[i], "OG historical raw word count differs");
        result.push(cp);
    }
    Ok(result)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub label: String,
    pub file: String,
    pub base_case: u64,
    pub scope: String,
    /// Actual captured clock, never replaced by the simulation timestamp.
    pub captured_at: At,
    /// Explicit even when it equals the captured timestamp.
    pub timestamp_override: u64,
    pub checkpoint: Checkpoint,
    /// Original raw fixture shape with only the declared raw/time replacements.
    pub snapshot: Value,
    pub expected: Value,
    pub gas: Option<u64>,
    pub extra_gas_controls: Vec<Value>,
    /// Preserves historical unsupported/classification/comparator labels and all
    /// error fields exactly, including absence of recursive error data.
    pub original: Value,
}

fn expected(v: &Value, recursive: bool) -> Result<()> {
    let fields: &[(&str, usize)] = if recursive {
        &[("holder_balance", 1), ("holder_hourly", 2), ("holder_daily", 1), ("pool_balance", 1)]
    } else {
        &[("balance", 1), ("hourly", 2), ("daily", 1)]
    };
    ensure!(
        v.as_object().context("OG expected object")?.len() == fields.len(),
        "OG expected getter scope differs"
    );
    for (field, width) in fields {
        let row = &v[*field];
        let object = row.as_object().context("OG getter expectation")?;
        ensure!(object.len() == 1, "ambiguous OG getter expectation");
        if let Some(values) = row.get("values") {
            let values = list(values)?;
            ensure!(values.len() == *width, "OG expected ABI width");
            for value in values {
                crate::data::uint(value)?;
            }
        } else {
            // Keep opaque provider errors as evidence. Parsing them is never
            // permission to map an arbitrary model refusal to a revert ABI.
            ensure!(row["rpc_error"].is_object(), "missing OG return/error evidence");
        }
    }
    Ok(())
}

/// Import one explicit raw/time override over one captured base. This accepts
/// opaque expected errors without interpreting them or feeding them into state.
pub fn control_from_raw(base: &Value, row: &Value, file: &str) -> Result<Control> {
    ensure!(
        matches!(file, "controls.json" | "preview.json" | "recursive.json"),
        "unsupported OG control family"
    );
    let base_cp = checkpoint_from_raw(base, descriptor(file)?.sha256)?;
    let base_case = number(&base["case"])?;
    if file == "controls.json" {
        ensure!(number(&row["base_case"])? == base_case, "OG control base case differs");
    } else {
        ensure!(base_case == 1, "OG preview/recursive base differs");
    }
    let name = text(&row["name"])?;
    ensure!(!name.is_empty(), "empty OG control name");
    let time = timestamp(&row["timestamp"])?;
    let changes = raw_rows(&row["state_diff"])?;
    let original_keys: BTreeSet<_> = base_cp.universe.iter().cloned().collect();
    ensure!(changes.keys().all(|k| original_keys.contains(k)), "OG control invents an undeclared raw key");
    let mut snapshot = base.clone();
    snapshot["timestamp"] = row["timestamp"].clone();
    // The source checkpoint's getter results are not results of this override.
    snapshot.as_object_mut().context("OG base object")?.remove("expected");
    for raw in snapshot["raw_state"].as_array_mut().context("OG raw state")? {
        let slot = Slot {
            contract: address(text(&raw["contract"])?)?,
            key: binding::hexword(&raw["key"])?,
        };
        if let Some(value) = changes.get(&slot) {
            raw["value"] = json!(format!("0x{}", hex::encode(value)));
        }
    }
    let recursive = file == "recursive.json";
    expected(&row["expected"], recursive)?;
    let gas = row.get("gas").map(number).transpose()?;
    let extra_gas_controls = row.get("extra_gas_controls").map(list).transpose()?.unwrap_or(&[]).to_vec();
    for extra in &extra_gas_controls {
        number(&extra["gas"])?;
        expected(&extra["expected"], recursive)?;
    }
    let input_sha256 = sha(&serde_json::to_vec(
        &json!({"case":base_case,"block":base["block"],"hash":base["hash"],"timestamp":row["timestamp"],"raw_state":snapshot["raw_state"]}),
    )?);
    let evidence = format!("{CONTROL_SCOPE}; {file}:{name}; source={}; raw_inputs={input_sha256}", descriptor(file)?.sha256);
    let checkpoint = import(&snapshot, &evidence, true)?;
    Ok(Control {
        label: format!("{file}:{name}"),
        file: file.into(),
        base_case,
        scope: CONTROL_SCOPE.into(),
        captured_at: base_cp.at,
        timestamp_override: time,
        checkpoint,
        snapshot,
        expected: row["expected"].clone(),
        gas,
        extra_gas_controls,
        original: row.clone(),
    })
}

pub fn controls() -> Result<Vec<Control>> {
    // Bind the fifth original too; its observations are used by the separate
    // continuation driver and never injected into these checkpoint values.
    binding::original("pool-range.json")?;
    let historical = binding::original("historical.json")?;
    let mut result = Vec::new();
    let simple = binding::original("controls.json")?;
    ensure!(list(&simple)?.len() == 43, "OG ordinary control count differs");
    for row in list(&simple)? {
        let case = number(&row["base_case"])?;
        let index = case.checked_sub(1).context("zero OG base case")?;
        let base = list(&historical)?.get(usize::try_from(index)?).context("unknown OG control base")?;
        result.push(control_from_raw(base, row, "controls.json")?);
    }
    for (file, count, base_words) in [("preview.json", 49, 201), ("recursive.json", 19, 162)] {
        let raw = binding::original(file)?;
        ensure!(
            list(&raw["controls"])?.len() == count && list(&raw["base"]["raw_state"])?.len() == base_words,
            "OG control/base scope differs"
        );
        for row in list(&raw["controls"])? {
            result.push(control_from_raw(&raw["base"], row, file)?);
        }
    }
    ensure!(
        result.len() == 111 && result.iter().map(|c| &c.label).collect::<BTreeSet<_>>().len() == 111,
        "OG control names/count differ"
    );
    Ok(result)
}

/// Count external getter observations without treating a tuple's two words as
/// separate calls. Internal preview returns and pool-range checks are separate.
pub fn external_observations() -> Result<usize> {
    let mut count = 0;
    let historical = binding::original("historical.json")?;
    for row in list(&historical)? {
        expected(&row["expected"], false)?;
        count += 3;
    }
    for control in controls()? {
        count += control.expected.as_object().context("control expected")?.len();
        for extra in control.extra_gas_controls {
            count += extra["expected"].as_object().context("extra expected")?.len();
        }
    }
    ensure!(count == 391, "OG external observation scope differs");
    Ok(count)
}

/// Strict uint256 decoding of saved value expectations for host comparators.
pub fn expected_values(v: &Value) -> Result<Vec<U256>> {
    list(&v["values"])?.iter().map(crate::data::uint).collect()
}
