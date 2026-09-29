//! Bounded host-only inputs for three historical calculated getters. No PB rows,
//! production layout, inferred zero, holder discovery or live qualification.
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub mod binding;
pub mod collect;
pub mod historical;
pub mod journal;
pub mod snapshots;
pub mod source;
use binding::{Binding, Model};

pub type Address = [u8; 20];
pub type Word = [u8; 32];
pub fn word(value: impl Into<U256>) -> Word {
    let mut bytes = [0; 32];
    value.into().to_big_endian(&mut bytes);
    bytes
}
pub fn value(bytes: &Word) -> U256 {
    U256::from_big_endian(bytes)
}
pub fn address(text: &str) -> Result<Address> {
    hex::decode(text.strip_prefix("0x").context("address prefix")?)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("address width"))
}
pub fn mapping(holder: Address, root: u64) -> Word {
    let mut preimage = [0; 64];
    preimage[12..32].copy_from_slice(&holder);
    preimage[32..].copy_from_slice(&word(root));
    erc20_balances::hash(&preimage)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub contract: Address,
    pub key: Word,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct At {
    pub number: u64,
    pub hash: Word,
    /// Missing only on a checkpoint whose capture did not include its parent.
    pub parent_hash: Option<Word>,
    pub timestamp: u64,
    /// Independent RPC checkpoints need not have an Extended producer version.
    pub producer_version: Option<u32>,
}
impl At {
    pub fn chain_clock(&self) -> Option<evm_retention::Clock> {
        Some(evm_retention::Clock {
            number: self.number,
            hash: self.hash.to_vec(),
            parent_hash: self.parent_hash?.to_vec(),
        })
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            self.number > 0 && self.producer_version.is_none_or(|v| matches!(v, 4 | 5)),
            "unsupported clock/version"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Origin {
    Checkpoint { evidence: String },
    Observed { ordinal: u64, source_sha256: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub slot: Slot,
    pub word: Word,
    pub at: At,
    pub epoch: u32,
    pub origin: Origin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub binding: Binding,
    pub at: At,
    pub holders: Vec<Address>,
    pub words: Vec<Fact>,
    pub evidence: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub holders: usize,
    pub words: usize,
    pub excluded: usize,
    pub writes: usize,
    pub undo_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            holders: 128,
            words: 4096,
            excluded: 128,
            writes: 16384,
            undo_depth: 64,
        }
    }
}
impl Limits {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.holders > 0
                && self.holders <= 4096
                && self.words > 0
                && self.words <= 65536
                && self.excluded <= 4096
                && self.writes > 0
                && self.writes <= 65536
                && self.undo_depth <= 1024,
            "invalid host bounds"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Write {
    pub slot: Slot,
    pub old: Word,
    pub new: Word,
    pub ordinal: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Code {
    pub contract: Address,
    pub old: Word,
    pub new: Word,
    pub ordinal: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockInput {
    pub at: At,
    pub source_sha256: String,
    pub writes: Vec<Write>,
    pub codes: Vec<Code>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Outcome {
    Known { amount: String, pending: Option<Pending> },
    Unknown { missing: Vec<Slot> },
    Suspended { epoch: u32, reason: String },
    ModelRefusal { reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pending {
    Known(String),
    ModelRefusal(String),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub holder: Address,
    pub at: At,
    pub binding_sha256: String,
    /// Digest of the complete finite raw-fact state, binding, clock, holder and
    /// current PB/checkpoint provenance. It is not a digest of a cached amount.
    pub input_sha256: String,
    pub source_sha256: Option<String>,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counters {
    pub blocks: u64,
    pub writes: u64,
    pub evaluations: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    binding: Binding,
    at: At,
    holders: Vec<Address>,
    words: Vec<Fact>,
    suspension: Option<String>,
    code_hashes: Vec<(Address, Word)>,
    counters: Counters,
    source_sha256: Option<String>,
    checkpoint_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    limits: Limits,
    state: State,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    limits: Limits,
    state: State,
    history: VecDeque<State>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Applied {
    pub evaluations: Vec<Evaluation>,
    pub changed: Vec<Address>,
    pub retained_writes: usize,
    pub suspended: bool,
}

impl Ledger {
    pub fn from_checkpoint(cp: Checkpoint, limits: Limits) -> Result<Self> {
        let checkpoint_sha256 = binding::sha(&serde_json::to_vec(&cp)?);
        limits.validate()?;
        cp.at.validate()?;
        cp.binding.validate()?;
        ensure!(cp.binding.activation_block <= cp.at.number, "checkpoint before model activation");
        ensure!(!cp.evidence.trim().is_empty(), "checkpoint evidence required");
        let mut holders = cp.holders;
        holders.sort();
        ensure!(
            holders.len() <= limits.holders && holders.windows(2).all(|p| p[0] != p[1]),
            "duplicate or excessive checkpoint holders"
        );
        let mut seen = BTreeSet::new();
        for fact in &cp.words {
            ensure!(seen.insert(fact.slot.clone()), "duplicate checkpoint slot");
            ensure!(fact.at == cp.at && fact.epoch == cp.binding.epoch, "checkpoint word identity differs");
            ensure!(fact.origin == Origin::Checkpoint { evidence: cp.evidence.clone() }, "checkpoint origin differs");
        }
        let mut words = cp.words;
        words.sort_by(|a, b| a.slot.cmp(&b.slot));
        let result = Self {
            limits,
            state: State {
                code_hashes: cp.binding.codes.iter().map(|c| (c.contract, c.runtime_hash)).collect(),
                binding: cp.binding,
                at: cp.at,
                holders,
                words,
                suspension: None,
                counters: Counters::default(),
                source_sha256: None,
                checkpoint_sha256,
            },
            history: VecDeque::new(),
        };
        result.validate_state()?;
        Ok(result)
    }
    pub fn binding(&self) -> &Binding {
        &self.state.binding
    }
    pub fn at(&self) -> &At {
        &self.state.at
    }
    pub fn facts(&self) -> &[Fact] {
        &self.state.words
    }
    pub fn holders(&self) -> &[Address] {
        &self.state.holders
    }
    pub fn counters(&self) -> &Counters {
        &self.state.counters
    }
    pub fn undo_snapshots(&self) -> usize {
        self.history.len()
    }
    fn map(&self) -> BTreeMap<Slot, Fact> {
        self.state.words.iter().cloned().map(|f| (f.slot.clone(), f)).collect()
    }
    fn closure(&self, words: &BTreeMap<Slot, Fact>) -> Result<BTreeSet<Slot>> {
        let binding = &self.state.binding;
        let token = binding.token();
        let mut keys = BTreeSet::new();
        let insert = |keys: &mut BTreeSet<Slot>, contract, key| {
            keys.insert(Slot { contract, key });
        };
        if binding.model == Model::Lbp {
            let dep = binding.dependency().expect("LBP dependency");
            for slot in [2, 6, 7, 8, 12, 17] {
                insert(&mut keys, dep, word(slot));
            }
            for h in &self.state.holders {
                insert(&mut keys, token, mapping(*h, 0));
                for root in [0, 9, 10, 11] {
                    insert(&mut keys, dep, mapping(*h, root));
                }
            }
        } else {
            let l = binding.reflection_layout().expect("reflection layout");
            for slot in [l.total_tokens, l.total_reflections, l.excluded_array] {
                insert(&mut keys, token, word(slot));
            }
            let mut accounts: BTreeSet<_> = self.state.holders.iter().copied().collect();
            if let Some(f) = words.get(&Slot {
                contract: token,
                key: word(l.excluded_array),
            }) {
                let length = value(&f.word);
                ensure!(length <= U256::from(self.limits.excluded), "exclusion list exceeds explicit host bound");
                let base = U256::from_big_endian(&erc20_balances::hash(&word(l.excluded_array)));
                for index in 0..length.as_usize() {
                    let key = word(base.overflowing_add(index.into()).0);
                    insert(&mut keys, token, key);
                    if let Some(f) = words.get(&Slot { contract: token, key }) {
                        accounts.insert(f.word[12..].try_into().unwrap());
                    }
                }
            }
            for h in accounts {
                for root in [l.reflections, l.tokens, l.excluded_flag] {
                    insert(&mut keys, token, mapping(h, root));
                }
            }
        }
        ensure!(keys.len() <= self.limits.words, "getter closure exceeds host bound");
        Ok(keys)
    }
    fn validate_state(&self) -> Result<()> {
        self.limits.validate()?;
        self.state.binding.validate()?;
        self.state.at.validate()?;
        ensure!(binding::is_sha(&self.state.checkpoint_sha256), "invalid checkpoint digest");
        ensure!(self.state.source_sha256.as_deref().is_none_or(binding::is_sha), "invalid source digest");
        ensure!(
            self.state.counters.blocks == 0 || self.state.source_sha256.is_some(),
            "applied state lacks PB provenance"
        );
        ensure!(self.state.binding.activation_block <= self.state.at.number, "state before binding activation");
        ensure!(
            self.state.holders.len() <= self.limits.holders && self.state.holders.windows(2).all(|p| p[0] < p[1]),
            "holder registry not canonical"
        );
        ensure!(
            self.state.words.len() <= self.limits.words && self.state.words.windows(2).all(|p| p[0].slot < p[1].slot),
            "raw facts not canonical"
        );
        let map = self.map();
        let closure = self.closure(&map)?;
        ensure!(self.state.code_hashes.len() == self.state.binding.codes.len(), "runtime state scope differs");
        for (current, bound) in self.state.code_hashes.iter().zip(&self.state.binding.codes) {
            ensure!(current.0 == bound.contract, "runtime state identity differs");
            ensure!(
                self.state.suspension.is_some() || current.1 == bound.runtime_hash,
                "active runtime differs from binding"
            );
        }
        for fact in &self.state.words {
            fact.at.validate()?;
            ensure!(
                fact.epoch == self.state.binding.epoch
                    && fact.at.number <= self.state.at.number
                    && fact.at.timestamp <= self.state.at.timestamp
                    && fact.at.number >= self.state.binding.activation_block
                    && fact.at.producer_version.is_none_or(|v| Some(v) == self.state.at.producer_version),
                "fact provenance outside epoch/clock"
            );
            ensure!(
                fact.at.number != self.state.at.number || fact.at == self.state.at,
                "current fact has another clock"
            );
            ensure!(closure.contains(&fact.slot), "fact outside finite getter input closure");
            match &fact.origin {
                Origin::Checkpoint { evidence } => ensure!(!evidence.trim().is_empty(), "checkpoint evidence required"),
                Origin::Observed { ordinal, source_sha256 } => ensure!(
                    *ordinal > 0 && fact.at.parent_hash.is_some() && fact.at.producer_version.is_some() && binding::is_sha(source_sha256),
                    "observation lacks storage ordinal/producer clock"
                ),
            }
        }
        Ok(())
    }
    fn outcome(&self, holder: Address) -> Result<Outcome> {
        Ok(if !self.state.holders.contains(&holder) {
            Outcome::Unknown { missing: vec![] }
        } else if let Some(reason) = &self.state.suspension {
            Outcome::Suspended {
                epoch: self.state.binding.epoch,
                reason: reason.clone(),
            }
        } else {
            self.calculate(holder)?
        })
    }
    fn state_input_digest(&self) -> Result<String> {
        Ok(binding::sha(&serde_json::to_vec(&(
            &self.state.binding,
            &self.state.holders,
            &self.state.at,
            &self.state.words,
            &self.state.code_hashes,
            &self.state.suspension,
            &self.state.checkpoint_sha256,
            &self.state.source_sha256,
        ))?))
    }
    pub fn evaluate(&self, holder: Address) -> Result<Evaluation> {
        self.evaluate_with_digest(holder, &self.state_input_digest()?)
    }
    fn evaluate_with_digest(&self, holder: Address, state_digest: &str) -> Result<Evaluation> {
        Ok(Evaluation {
            holder,
            at: self.state.at.clone(),
            binding_sha256: self.state.binding.digest()?,
            input_sha256: binding::sha(&serde_json::to_vec(&(holder, state_digest))?),
            source_sha256: self.state.source_sha256.clone(),
            outcome: self.outcome(holder)?,
        })
    }
    fn calculate(&self, holder: Address) -> Result<Outcome> {
        let b = &self.state.binding;
        let words = self.map();
        let token = b.token();
        let mut required = BTreeSet::new();
        let add = |set: &mut BTreeSet<Slot>, contract, key| {
            set.insert(Slot { contract, key });
        };
        if b.model == Model::Lbp {
            let dep = b.dependency().unwrap();
            for s in [2, 6, 7, 8, 12, 17] {
                add(&mut required, dep, word(s));
            }
            add(&mut required, token, mapping(holder, 0));
            for s in [0, 9, 10, 11] {
                add(&mut required, dep, mapping(holder, s));
            }
        } else {
            let l = b.reflection_layout().unwrap();
            for s in [l.reflections, l.tokens, l.excluded_flag] {
                add(&mut required, token, mapping(holder, s));
            }
            let excluded = words
                .get(&Slot {
                    contract: token,
                    key: mapping(holder, l.excluded_flag),
                })
                .map(|f| f.word[31] != 0);
            if excluded == Some(false) {
                for s in [l.total_reflections, l.total_tokens, l.excluded_array] {
                    add(&mut required, token, word(s));
                }
                if let Some(f) = words.get(&Slot {
                    contract: token,
                    key: word(l.excluded_array),
                }) {
                    let n = value(&f.word);
                    ensure!(n <= self.limits.excluded.into(), "exclusion bound");
                    let base = U256::from_big_endian(&erc20_balances::hash(&word(l.excluded_array)));
                    for i in 0..n.as_usize() {
                        let key = word(base.overflowing_add(i.into()).0);
                        add(&mut required, token, key);
                        if let Some(f) = words.get(&Slot { contract: token, key }) {
                            let h = f.word[12..].try_into().unwrap();
                            for s in [l.reflections, l.tokens] {
                                add(&mut required, token, mapping(h, s));
                            }
                        }
                    }
                }
            }
        }
        let missing: Vec<_> = required.into_iter().filter(|k| !words.contains_key(k)).collect();
        if !missing.is_empty() {
            return Ok(Outcome::Unknown { missing });
        }
        let result: Result<(U256, Option<Pending>)> = if b.model == Model::Lbp {
            let raw = words
                .iter()
                .map(|(k, f)| ((format!("0x{}", hex::encode(k.contract)), format!("0x{}", hex::encode(k.key))), value(&f.word)))
                .collect();
            crate::lbp_rewards::fixture::decode(
                &raw,
                &format!("0x{}", hex::encode(holder)),
                &b.exemptions.iter().map(|a| format!("0x{}", hex::encode(a))).collect::<Vec<_>>(),
            )
            .and_then(|(g, h)| {
                // Keep pure-model short circuits. The existing decoder shifts
                // the reserve lane without masking the unused high byte; only
                // calculations needing that global state refuse this padding.
                if words[&Slot {
                    contract: b.dependency().unwrap(),
                    key: word(6),
                }]
                    .word[0]
                    != 0
                    && !h.zero_or_dead
                    && (!h.shares.is_zero() || h.is_node)
                {
                    let reason = "unsupported nonzero unused hLBP packed padding";
                    return if h.exempt {
                        Ok((h.raw, Some(Pending::ModelRefusal(reason.into()))))
                    } else {
                        anyhow::bail!(reason)
                    };
                }
                let amount = h.balance(&g, self.state.at.timestamp)?;
                let pending = match h.pending(&g, self.state.at.timestamp) {
                    Ok(v) => Pending::Known(v.to_string()),
                    Err(e) => Pending::ModelRefusal(e.to_string()),
                };
                Ok((amount, Some(pending)))
            })
        } else {
            let raw = words.iter().map(|(k, f)| (format!("0x{}", hex::encode(k.key)), value(&f.word))).collect();
            crate::reflection::fixture::decode(&b.reflection_layout().unwrap(), &format!("0x{}", hex::encode(holder)), &raw)
                .and_then(|s| Ok((s.balance()?, None)))
        };
        Ok(match result {
            Ok((amount, pending)) => Outcome::Known {
                amount: amount.to_string(),
                pending,
            },
            Err(e) => Outcome::ModelRefusal { reason: e.to_string() },
        })
    }
    pub fn apply(&mut self, input: &BlockInput) -> Result<Applied> {
        let mut staged = self.stage();
        let applied = staged.apply_inner(input)?;
        self.reserve_history()?;
        self.commit(staged);
        Ok(applied)
    }
    fn stage(&self) -> Self {
        Self {
            limits: self.limits,
            state: self.state.clone(),
            history: VecDeque::new(),
        }
    }
    fn reserve_history(&mut self) -> Result<()> {
        if self.limits.undo_depth > 0 {
            self.history.try_reserve(1).context("undo ring allocation failed")?;
        }
        Ok(())
    }
    // Called only after all fallible validations and allocations succeed.
    fn commit(&mut self, staged: Self) {
        let before = std::mem::replace(&mut self.state, staged.state);
        if self.limits.undo_depth > 0 {
            self.history.push_back(before);
            while self.history.len() > self.limits.undo_depth {
                self.history.pop_front();
            }
        }
    }
    fn apply_inner(&mut self, input: &BlockInput) -> Result<Applied> {
        input.at.validate()?;
        ensure!(
            input.at.parent_hash.is_some() && input.at.producer_version.is_some(),
            "applied block requires complete producer clock"
        );
        ensure!(
            self.state.at.number.checked_add(1) == Some(input.at.number) && input.at.parent_hash == Some(self.state.at.hash),
            "block gap/fork; undo first"
        );
        ensure!(
            self.state.at.producer_version.is_none_or(|v| input.at.producer_version == Some(v)) && input.at.timestamp >= self.state.at.timestamp,
            "version or timestamp discontinuity"
        );
        ensure!(binding::is_sha(&input.source_sha256), "source block digest required");
        ensure!(
            input.writes.len() <= self.limits.writes && input.codes.len() <= self.limits.writes,
            "input effect bound exceeded"
        );
        let previous = self
            .state
            .holders
            .iter()
            .map(|h| Ok((*h, self.outcome(*h)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut words = self.map();
        let mut physical = BTreeMap::<Slot, (u64, Word)>::new();
        for w in &input.writes {
            if !self.state.binding.protected(w.slot.contract) {
                continue;
            }
            ensure!(w.ordinal > 0, "storage effect lacks ordinal");
            if let Some((ordinal, prior)) = physical.get(&w.slot) {
                ensure!(w.ordinal > *ordinal && w.old == *prior, "ambiguous/discontinuous physical storage chain");
            } else if let Some(f) = words.get(&w.slot) {
                ensure!(w.old == f.word, "known storage discontinuity");
            }
            physical.insert(w.slot.clone(), (w.ordinal, w.new));
            words.insert(
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
        }
        let closure = self.closure(&words)?;
        words.retain(|key, _| closure.contains(key));
        let retained_writes = input.writes.iter().filter(|w| closure.contains(&w.slot)).count();
        let mut codes: BTreeMap<Address, (u64, Word)> = BTreeMap::new();
        for c in &input.codes {
            if !self.state.binding.protected(c.contract) {
                continue;
            }
            ensure!(c.ordinal > 0, "code effect lacks ordinal");
            if let Some((ordinal, last)) = codes.get(&c.contract) {
                ensure!(c.ordinal > *ordinal && c.old == *last, "ambiguous/discontinuous code chain");
            } else {
                ensure!(
                    self.state.code_hashes.iter().find(|(a, _)| *a == c.contract).map(|(_, h)| *h) == Some(c.old),
                    "protected code old hash mismatch"
                );
            }
            codes.insert(c.contract, (c.ordinal, c.new));
            self.state.code_hashes.iter_mut().find(|(a, _)| *a == c.contract).unwrap().1 = c.new;
            if c.old != c.new {
                self.state.suspension = Some(format!("protected runtime changed at {}:{}", input.at.number, c.ordinal));
            }
        }
        self.state.words = words.into_values().collect();
        self.state.at = input.at.clone();
        self.state.source_sha256 = Some(input.source_sha256.clone());
        self.state.counters.blocks = self.state.counters.blocks.checked_add(1).context("block count overflow")?;
        self.state.counters.writes = self.state.counters.writes.checked_add(retained_writes as u64).context("write count overflow")?;
        self.state.counters.evaluations = self
            .state
            .counters
            .evaluations
            .checked_add(self.state.holders.len() as u64)
            .context("evaluation count overflow")?;
        self.validate_state()?;
        let state_digest = self.state_input_digest()?;
        let evaluations = self
            .state
            .holders
            .iter()
            .map(|h| self.evaluate_with_digest(*h, &state_digest))
            .collect::<Result<Vec<_>>>()?;
        let changed = evaluations.iter().filter(|e| previous[&e.holder] != e.outcome).map(|e| e.holder).collect();
        Ok(Applied {
            evaluations,
            changed,
            retained_writes,
            suspended: self.state.suspension.is_some(),
        })
    }
    pub fn undo(&mut self, number: u64, hash: Word) -> Result<usize> {
        let count = self.state.at.number.checked_sub(number).context("undo target after current clock")?;
        ensure!(count > 0 && count <= self.history.len() as u64, "undo target outside bounded history");
        let target = &self.history[self.history.len() - count as usize];
        ensure!(target.at.number == number && target.at.hash == hash, "undo target hash mismatch");
        for _ in 0..count {
            self.state = self.history.pop_back().unwrap();
        }
        Ok(count as usize)
    }
    pub fn resume(&mut self, cp: Checkpoint) -> Result<()> {
        ensure!(self.state.suspension.is_some(), "resume requires suspended epoch");
        ensure!(
            cp.binding.model == self.state.binding.model && cp.binding.epoch > self.state.binding.epoch && cp.at == self.state.at,
            "resume requires same boundary and newer model epoch"
        );
        ensure!(
            cp.binding.codes.iter().map(|c| (c.contract, c.runtime_hash)).collect::<Vec<_>>() == self.state.code_hashes,
            "resume runtime differs from observed code state"
        );
        let staged = Self::from_checkpoint(cp, self.limits)?;
        ensure!(
            staged.state.words.len() == staged.closure(&staged.map())?.len(),
            "resume requires complete finite raw facts"
        );
        *self = staged;
        Ok(())
    }
    /// Restart snapshot: preserves exact facts/counters, intentionally no undo
    /// history. The caller supplies an independently retained digest and binding.
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&Snapshot {
            schema: 1,
            limits: self.limits,
            state: self.state.clone(),
        })?)
    }
    pub fn restore(bytes: &[u8], expected_sha256: &str, expected: &Binding) -> Result<Self> {
        ensure!(binding::sha(bytes) == expected_sha256, "snapshot digest mismatch");
        let snapshot: Snapshot = serde_json::from_slice(bytes)?;
        ensure!(snapshot.schema == 1 && snapshot.state.binding == *expected, "snapshot identity mismatch");
        let result = Self {
            limits: snapshot.limits,
            state: snapshot.state,
            history: VecDeque::new(),
        };
        result.validate_state()?;
        Ok(result)
    }
}
/// Atomic across an explicitly finite group; no adapter can commit a prefix.
pub fn apply_all(ledgers: &mut [Ledger], input: &BlockInput) -> Result<Vec<Applied>> {
    let mut staged: Vec<_> = ledgers.iter().map(Ledger::stage).collect();
    let rows = staged.iter_mut().map(|l| l.apply_inner(input)).collect::<Result<Vec<_>>>()?;
    // Capacity alone is not semantic state. Preflight every allocation before
    // committing any facts, clock, counters or history entries.
    for ledger in ledgers.iter_mut() {
        ledger.reserve_history()?;
    }
    for (ledger, next) in ledgers.iter_mut().zip(staged) {
        ledger.commit(next);
    }
    Ok(rows)
}
