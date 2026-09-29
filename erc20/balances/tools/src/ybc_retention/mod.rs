//! Closed host reference for finite YBC raw inputs. No production layout,
//! inferred zero, on-chain admission, or change to the three PR105 bindings.
pub mod binding;
pub mod collect;
pub mod decode;
pub mod historical;
pub mod journal;
pub mod snapshots;

use crate::calculated_retention::binding::{is_sha, sha};
pub use crate::calculated_retention::{address, value, word, Address, At, BlockInput, Fact, Origin, Slot, Word};
use anyhow::{ensure, Context, Result};
pub use binding::Binding;
pub use decode::{Evaluation, Metric, Reward};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub holders: usize,
    pub keys: usize,
    pub effects: usize,
    pub undo: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            holders: 12,
            keys: 8192,
            effects: 16384,
            undo: 16,
        }
    }
}
impl Limits {
    fn validate(self) -> Result<()> {
        ensure!(
            self.holders > 0 && self.holders <= 128 && self.keys > 0 && self.keys <= 32768 && self.effects > 0 && self.effects <= 65536 && self.undo <= 128,
            "YBC host bounds"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub binding: Binding,
    pub at: At,
    pub holders: Vec<Address>,
    /// Permitted finite key names. Membership is never evidence of a value.
    pub universe: Vec<Slot>,
    pub facts: Vec<Fact>,
    pub evidence: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counters {
    pub blocks: u64,
    pub retained_writes: u64,
    pub evaluations: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    binding: Binding,
    at: At,
    holders: Vec<Address>,
    universe: Vec<Slot>,
    facts: Vec<Fact>,
    codes: Vec<(Address, Word)>,
    suspension: Option<String>,
    counters: Counters,
    checkpoint_sha256: String,
    source_sha256: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    limits: Limits,
    state: State,
    history: VecDeque<State>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    limits: Limits,
    state: State,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Applied {
    pub retained_writes: usize,
    pub evaluations: Vec<Evaluation>,
}
fn clock(at: &At) -> Result<()> {
    ensure!(at.number > 0 && at.producer_version.is_none_or(|v| matches!(v, 4 | 5)), "YBC clock/version");
    Ok(())
}
impl Ledger {
    pub fn from_checkpoint(cp: Checkpoint, limits: Limits) -> Result<Self> {
        ensure!(!cp.evidence.trim().is_empty(), "checkpoint evidence required");
        for f in &cp.facts {
            ensure!(
                f.at == cp.at && f.epoch == cp.binding.epoch && f.origin == Origin::Checkpoint { evidence: cp.evidence.clone() },
                "checkpoint fact origin differs"
            );
        }
        let checkpoint_sha256 = sha(&serde_json::to_vec(&cp)?);
        let mut state = State {
            codes: binding::codes().to_vec(),
            binding: cp.binding,
            at: cp.at,
            holders: cp.holders,
            universe: cp.universe,
            facts: cp.facts,
            suspension: None,
            counters: Counters::default(),
            checkpoint_sha256,
            source_sha256: None,
        };
        state.holders.sort();
        state.universe.sort();
        state.facts.sort_by(|a, b| a.slot.cmp(&b.slot));
        let ledger = Self {
            limits,
            state,
            history: VecDeque::new(),
        };
        ledger.validate()?;
        if let Some(p) = ledger.fact(&binding::pointer()) {
            ensure!(p.word == binding::helper_word(), "checkpoint helper pointer mismatch");
        }
        Ok(ledger)
    }
    fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        self.state.binding.validate()?;
        clock(&self.state.at)?;
        ensure!(self.state.at.number >= self.state.binding.activation_block, "checkpoint before activation");
        let s = &self.state;
        ensure!(
            !s.holders.is_empty() && s.holders.len() <= self.limits.holders && s.holders.windows(2).all(|p| p[0] < p[1]),
            "holder registry order/bound"
        );
        ensure!(
            s.universe.len() <= self.limits.keys
                && s.universe.windows(2).all(|p| p[0] < p[1])
                && s.universe.iter().all(|k| binding::protected(k.contract) && k.contract != binding::helper()),
            "finite key universe order/bound"
        );
        ensure!(
            s.facts.len() <= self.limits.keys && s.facts.windows(2).all(|p| p[0].slot < p[1].slot),
            "fact order/bound"
        );
        ensure!(
            is_sha(&s.checkpoint_sha256) && s.source_sha256.as_ref().is_none_or(|v| is_sha(v)),
            "state provenance digest"
        );
        ensure!(
            s.codes.len() == 3 && s.codes.iter().map(|c| c.0).eq(binding::codes().iter().map(|c| c.0)),
            "code identity order"
        );
        if s.suspension.is_none() {
            ensure!(s.codes == binding::codes(), "unsuspended runtime differs");
        }
        for f in &s.facts {
            clock(&f.at)?;
            ensure!(
                s.universe.binary_search(&f.slot).is_ok()
                    && f.epoch == s.binding.epoch
                    && f.at.number >= s.binding.activation_block
                    && f.at.number <= s.at.number
                    && f.at.timestamp <= s.at.timestamp
                    && f.at.producer_version.is_none_or(|v| s.at.producer_version == Some(v)),
                "fact outside identity/universe"
            );
            if f.at.number == s.at.number {
                ensure!(
                    f.at == s.at
                        || (f.at.hash == s.at.hash && f.at.timestamp == s.at.timestamp && f.at.producer_version.is_none() && f.at.parent_hash.is_none()),
                    "same-height fact differs"
                );
            }
            match &f.origin {
                Origin::Checkpoint { evidence } => ensure!(!evidence.trim().is_empty(), "checkpoint origin"),
                Origin::Observed { ordinal, source_sha256 } => ensure!(
                    *ordinal > 0 && is_sha(source_sha256) && f.at.parent_hash.is_some() && f.at.producer_version.is_some(),
                    "observed origin"
                ),
            }
        }
        Ok(())
    }
    pub fn at(&self) -> &At {
        &self.state.at
    }
    pub fn binding(&self) -> &Binding {
        &self.state.binding
    }
    pub fn holders(&self) -> &[Address] {
        &self.state.holders
    }
    pub fn universe(&self) -> &[Slot] {
        &self.state.universe
    }
    pub fn facts(&self) -> &[Fact] {
        &self.state.facts
    }
    pub fn counters(&self) -> &Counters {
        &self.state.counters
    }
    pub fn suspension(&self) -> Option<&str> {
        self.state.suspension.as_deref()
    }
    pub fn fact(&self, slot: &Slot) -> Option<&Fact> {
        self.state.facts.binary_search_by(|f| f.slot.cmp(slot)).ok().map(|i| &self.state.facts[i])
    }
    pub fn evaluate(&self, holder: Address) -> Result<Evaluation> {
        ensure!(self.state.holders.binary_search(&holder).is_ok(), "unregistered holder");
        decode::evaluate(self, holder, sha(&serde_json::to_vec(&self.state)?))
    }
    pub fn evaluate_all(&self) -> Result<Vec<Evaluation>> {
        let digest = sha(&serde_json::to_vec(&self.state)?);
        self.state.holders.iter().map(|h| decode::evaluate(self, *h, digest.clone())).collect()
    }
    pub fn apply(&mut self, input: &BlockInput) -> Result<Applied> {
        let mut staged = Self {
            limits: self.limits,
            state: self.state.clone(),
            history: VecDeque::new(),
        };
        let result = staged.apply_inner(input)?;
        if self.limits.undo > 0 {
            self.history.try_reserve(1).context("undo allocation")?;
        }
        let old = std::mem::replace(&mut self.state, staged.state);
        if self.limits.undo > 0 {
            self.history.push_back(old);
            while self.history.len() > self.limits.undo {
                self.history.pop_front();
            }
        }
        Ok(result)
    }
    fn apply_inner(&mut self, input: &BlockInput) -> Result<Applied> {
        clock(&input.at)?;
        ensure!(
            input.at.producer_version.is_some()
                && input.at.parent_hash == Some(self.state.at.hash)
                && self.state.at.number.checked_add(1) == Some(input.at.number),
            "block gap/fork"
        );
        ensure!(
            input.at.timestamp >= self.state.at.timestamp && self.state.at.producer_version.is_none_or(|v| input.at.producer_version == Some(v)),
            "timestamp/producer discontinuity"
        );
        ensure!(
            is_sha(&input.source_sha256) && input.writes.len().checked_add(input.codes.len()).is_some_and(|n| n <= self.limits.effects),
            "input digest/effect bound"
        );
        let mut facts: BTreeMap<_, _> = self.state.facts.iter().cloned().map(|f| (f.slot.clone(), f)).collect();
        let mut physical = BTreeMap::<Slot, (u64, Word)>::new();
        let mut retained_writes = 0;
        for w in &input.writes {
            ensure!(binding::protected(w.slot.contract), "foreign input account");
            ensure!(w.ordinal > 0, "missing storage ordinal");
            if let Some((ord, last)) = physical.get(&w.slot) {
                ensure!(w.ordinal > *ord && w.old == *last, "same-key order/continuity");
            } else if let Some(f) = facts.get(&w.slot) {
                ensure!(w.old == f.word, "retained word discontinuity");
            }
            physical.insert(w.slot.clone(), (w.ordinal, w.new));
            if (w.slot.contract == binding::helper() && w.old != w.new)
                || (w.slot == binding::pointer() && (w.old != binding::helper_word() || w.new != binding::helper_word()))
            {
                self.state.suspension = Some(format!("unreviewed helper state/pointer at {}:{}", input.at.number, w.ordinal));
            }
            if self.state.universe.binary_search(&w.slot).is_ok() {
                facts.insert(
                    w.slot.clone(),
                    Fact {
                        slot: w.slot.clone(),
                        word: w.new,
                        at: input.at.clone(),
                        epoch: self.state.binding.epoch,
                        origin: Origin::Observed {
                            ordinal: w.ordinal,
                            source_sha256: input.source_sha256.clone(),
                        },
                    },
                );
                retained_writes += 1;
            }
        }
        let mut changes = BTreeMap::<Address, (u64, Word)>::new();
        for c in &input.codes {
            ensure!(binding::protected(c.contract) && c.ordinal > 0, "code account/ordinal");
            let entry = self.state.codes.iter_mut().find(|v| v.0 == c.contract).unwrap();
            if let Some((ord, hash)) = changes.get(&c.contract) {
                ensure!(c.ordinal > *ord && c.old == *hash, "code order/continuity");
            } else {
                ensure!(c.old == entry.1, "runtime old hash");
            }
            changes.insert(c.contract, (c.ordinal, c.new));
            entry.1 = c.new;
            if c.old != c.new {
                self.state.suspension = Some(format!("runtime changed at {}:{}", input.at.number, c.ordinal));
            }
        }
        self.state.facts = facts.into_values().collect();
        self.state.at = input.at.clone();
        self.state.source_sha256 = Some(input.source_sha256.clone());
        self.state.counters.blocks = self.state.counters.blocks.checked_add(1).context("block counter overflow")?;
        self.state.counters.retained_writes = self
            .state
            .counters
            .retained_writes
            .checked_add(retained_writes as u64)
            .context("write counter overflow")?;
        self.state.counters.evaluations = self
            .state
            .counters
            .evaluations
            .checked_add(self.state.holders.len() as u64)
            .context("evaluation counter overflow")?;
        self.validate()?;
        Ok(Applied {
            retained_writes,
            evaluations: self.evaluate_all()?,
        })
    }
    pub fn undo(&mut self, number: u64, hash: Word) -> Result<usize> {
        let n = self.state.at.number.checked_sub(number).context("undo ahead")?;
        ensure!(n > 0 && n <= self.history.len() as u64, "undo outside journal");
        let target = &self.history[self.history.len() - n as usize];
        ensure!(target.at.number == number && target.at.hash == hash, "undo identity");
        for _ in 0..n {
            self.state = self.history.pop_back().unwrap();
        }
        Ok(n as usize)
    }
    /// New independently initialized epoch. No prior fact or undo entry carries.
    pub fn resume(&mut self, cp: Checkpoint) -> Result<()> {
        ensure!(
            self.state.suspension.is_some()
                && cp.at == self.state.at
                && cp.binding.epoch > self.state.binding.epoch
                && cp.holders == self.state.holders
                && cp.universe == self.state.universe
                && self.state.codes == binding::codes(),
            "reset boundary/epoch/registry/runtime"
        );
        let next = Self::from_checkpoint(cp, self.limits)?;
        ensure!(next.fact(&binding::pointer()).is_some(), "reset pointer missing");
        ensure!(
            next.evaluate_all()?.iter().all(|v| !v.pending.is_missing() && !v.observable.is_missing()),
            "reset missing currently required inputs"
        );
        *self = next;
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&Snapshot {
            schema: 1,
            limits: self.limits,
            state: self.state.clone(),
        })?)
    }
    pub fn restore(raw: &[u8], digest: &str, binding: &Binding) -> Result<Self> {
        ensure!(sha(raw) == digest, "snapshot digest");
        let s: Snapshot = serde_json::from_slice(raw)?;
        ensure!(s.schema == 1 && s.state.binding == *binding, "snapshot binding/schema");
        let l = Self {
            limits: s.limits,
            state: s.state,
            history: VecDeque::new(),
        };
        l.validate()?;
        Ok(l)
    }
}
