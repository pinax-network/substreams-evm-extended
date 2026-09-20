//! Retained holder state for consumers of the RPC-free balance packages.
//!
//! A map emits only the holders and market words a block wrote. Everything a
//! consumer knows about an untouched holder comes from retained state, and
//! that state has an origin: an explicit checkpoint at the block before the
//! first delivered block, the EVM-initial zero storage of a token whose
//! creation the consumer observed, or an emitted row. An address with none of
//! those is **unknown**, never zero. This crate keeps the cases distinct:
//!
//! | Case | Representation |
//! | --- | --- |
//! | missing (cold) | `Lookup::Unknown` |
//! | uninitialized token | `Lookup::Unknown` for every holder of that token |
//! | reverted effect | never a row, so never an entry |
//! | known zero | `Entry.value == "0"` |
//! | unsupported token / model | `Lookup::Unsupported` |
//! | invalidated or suspended epoch | `Lookup::Suspended` |
//!
//! Blocks must be contiguous by number and parent hash; a fork is handled by
//! `undo`, which restores the exact prior entries from a bounded journal.
//! The report counts emitted rows, initialized holders by origin, known
//! zeros, cold lookups and undone blocks separately, and never claims a
//! global holder set unless independent enumeration evidence was attached.
//! This is host-side consumer tooling; it is not a map and holds no RPC.
use proto::pb::evm::balance_state::v1 as state;
use proto::pb::evm::balances::v1 as balances;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn err<T>(m: impl Into<String>) -> Result<T> {
    Err(Error(m.into()))
}

/// `(contract, holder)`; `contract == None` is the native balance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Key {
    pub contract: Option<Vec<u8>>,
    pub address: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Origin {
    /// Independently verified value at `block` (the block before the first
    /// applied one), with the evidence reference the consumer was given.
    Checkpoint { block: u64, evidence: String },
    /// EVM-initial zero storage of a token whose creation was observed at
    /// `block`; only valid for holders that cannot precede the creation.
    DeploymentZero { block: u64 },
    /// An emitted end-of-block row.
    Observed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Exact decimal integer; `"0"` is a known zero. Signed for basis kinds
    /// that carry sign (Comet principal).
    pub value: String,
    pub origin: Origin,
    /// Block at which the entry became known.
    pub since: u64,
    /// Block of the last emitted change (`since` for seeded, unchanged entries).
    pub updated: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup<'a> {
    Known(&'a Entry),
    Unknown,
    Unsupported(&'a str),
    Suspended { epoch: u32, reason: i32 },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clock {
    pub number: u64,
    pub hash: Vec<u8>,
    pub parent_hash: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suspension {
    pub epoch: u32,
    pub reason: i32,
    pub block: u64,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Applied {
    pub rows: usize,
    pub new_holders: usize,
    pub zero_rows: usize,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub blocks_applied: u64,
    pub first_block: Option<u64>,
    pub last_block: Option<u64>,
    pub last_hash: Option<String>,
    pub emitted_rows: u64,
    pub undone_blocks: u64,
    pub initialized_holders: usize,
    pub observed_holders: usize,
    pub checkpoint_seeded_holders: usize,
    pub deployment_seeded_holders: usize,
    pub known_zero_holders: usize,
    pub cold_unknown_lookups: u64,
    pub unsupported_contracts: usize,
    pub suspended_markets: usize,
    /// Present only when `attach_enumeration` supplied independent evidence;
    /// the retained set is never reported as the global holder set.
    pub enumerated: Option<Enumeration>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Enumeration {
    pub contract: Vec<u8>,
    pub holders: usize,
    pub block: u64,
    pub evidence: String,
    pub retained_missing: usize,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    pub reference_rows: usize,
    pub matches: usize,
    pub known_zero_matches: usize,
    pub mismatches: Vec<(Key, String, String)>,
    pub unknown: usize,
    pub unknown_nonzero: usize,
    pub unsupported: usize,
    pub suspended: usize,
}
impl Comparison {
    /// Bounded parity: every reference row that the ledger knows agrees.
    /// Unknown rows are coverage gaps, never matches and never failures.
    pub fn status(&self) -> &'static str {
        if !self.mismatches.is_empty() {
            "mismatch"
        } else if self.unknown > 0 || self.unsupported > 0 || self.suspended > 0 {
            "coverage_gap"
        } else {
            "bounded_parity"
        }
    }
}

fn valid_decimal(s: &str, signed: bool) -> bool {
    let digits = if signed { s.strip_prefix('-').unwrap_or(s) } else { s };
    !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) && (digits == "0" || !digits.starts_with('0')) && s != "-0"
}

#[derive(Debug, Clone)]
struct Journal {
    clock: Clock,
    previous: Vec<(Key, Option<Entry>)>,
    previous_suspensions: Vec<(Vec<u8>, Option<Suspension>)>,
    rows: u64,
}

/// Retained holder state with a bounded undo journal.
#[derive(Debug, Clone)]
pub struct Ledger {
    entries: BTreeMap<Key, Entry>,
    unsupported: BTreeMap<Vec<u8>, String>,
    suspended: BTreeMap<Vec<u8>, Suspension>,
    enumerations: Vec<Enumeration>,
    last: Option<Clock>,
    first_block: Option<u64>,
    journal: VecDeque<Journal>,
    undo_depth: usize,
    blocks_applied: u64,
    emitted_rows: u64,
    undone_blocks: u64,
    cold_lookups: std::cell::Cell<u64>,
}

impl Ledger {
    pub fn new(undo_depth: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            unsupported: BTreeMap::new(),
            suspended: BTreeMap::new(),
            enumerations: Vec::new(),
            last: None,
            first_block: None,
            journal: VecDeque::new(),
            undo_depth,
            blocks_applied: 0,
            emitted_rows: 0,
            undone_blocks: 0,
            cold_lookups: std::cell::Cell::new(0),
        }
    }

    /// Seed a verified value at `block`; every seed must sit at the block
    /// before the first applied block, which `apply` enforces.
    pub fn seed_checkpoint(&mut self, key: Key, value: &str, block: u64, evidence: &str) -> Result<()> {
        if self.last.is_some() {
            return err("checkpoints are seeded before the first block");
        }
        if !valid_decimal(value, true) {
            return err(format!("invalid checkpoint value `{value}`"));
        }
        if evidence.is_empty() {
            return err("checkpoint evidence reference required");
        }
        if self.entries.contains_key(&key) {
            return err("holder already seeded");
        }
        self.entries.insert(
            key,
            Entry {
                value: value.to_string(),
                origin: Origin::Checkpoint {
                    block,
                    evidence: evidence.to_string(),
                },
                since: block,
                updated: block,
            },
        );
        Ok(())
    }

    /// Seed EVM-initial zeros for a token created at `creation_block`, for
    /// holders that cannot hold it before creation. Never for an existing
    /// token: that would infer zero from an absent write.
    pub fn seed_deployment_zero(&mut self, contract: &[u8], holders: &BTreeSet<Vec<u8>>, creation_block: u64) -> Result<usize> {
        if self.entries.keys().any(|k| k.contract.as_deref() == Some(contract)) {
            return err("deployment token already has holder state");
        }
        if let Some(last) = &self.last {
            if creation_block > last.number {
                return err("deployment block is after the last applied block");
            }
        }
        for holder in holders {
            self.entries.insert(
                Key {
                    contract: Some(contract.to_vec()),
                    address: holder.clone(),
                },
                Entry {
                    value: "0".into(),
                    origin: Origin::DeploymentZero { block: creation_block },
                    since: creation_block,
                    updated: creation_block,
                },
            );
        }
        Ok(holders.len())
    }

    pub fn mark_unsupported(&mut self, contract: &[u8], reason: &str) {
        self.unsupported.insert(contract.to_vec(), reason.to_string());
    }

    /// Independent enumeration evidence for one contract at one block. The
    /// report carries it verbatim next to the retained counts.
    pub fn attach_enumeration(&mut self, contract: &[u8], holders: &BTreeSet<Vec<u8>>, block: u64, evidence: &str) -> Result<()> {
        if evidence.is_empty() || holders.is_empty() {
            return err("enumeration needs holders and an evidence reference");
        }
        let retained_missing = holders
            .iter()
            .filter(|h| {
                !self.entries.contains_key(&Key {
                    contract: Some(contract.to_vec()),
                    address: (*h).clone(),
                })
            })
            .count();
        self.enumerations.push(Enumeration {
            contract: contract.to_vec(),
            holders: holders.len(),
            block,
            evidence: evidence.to_string(),
            retained_missing,
        });
        Ok(())
    }

    fn check_clock(&self, clock: &Clock) -> Result<()> {
        if clock.hash.len() != 32 || clock.parent_hash.len() != 32 {
            return err("block identity must be 32-byte hashes");
        }
        match &self.last {
            Some(last) => {
                if clock.number != last.number + 1 {
                    return err(format!("block gap: expected {}, got {}", last.number + 1, clock.number));
                }
                if clock.parent_hash != last.hash {
                    return err(format!("fork at block {}: parent does not match retained hash; undo first", clock.number));
                }
            }
            None => {
                for (key, entry) in &self.entries {
                    if let Origin::Checkpoint { block, .. } = entry.origin {
                        if block + 1 != clock.number {
                            return err(format!(
                                "checkpoint for 0x{} is at block {}, not the parent of the first block {}",
                                hex_of(&key.address),
                                block,
                                clock.number
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn begin(&mut self, clock: &Clock) -> Journal {
        Journal {
            clock: clock.clone(),
            previous: Vec::new(),
            previous_suspensions: Vec::new(),
            rows: 0,
        }
    }
    fn commit(&mut self, journal: Journal) {
        self.emitted_rows += journal.rows;
        self.blocks_applied += 1;
        self.first_block.get_or_insert(journal.clock.number);
        self.last = Some(journal.clock.clone());
        self.journal.push_back(journal);
        while self.journal.len() > self.undo_depth {
            self.journal.pop_front();
        }
    }
    fn set(&mut self, journal: &mut Journal, key: Key, entry: Entry) {
        let previous = self.entries.insert(key.clone(), entry);
        if !journal.previous.iter().any(|(k, _)| *k == key) {
            journal.previous.push((key, previous));
        }
    }

    /// Apply one block of `evm.balances.v1` rows. Rows for unsupported
    /// contracts fail: the ledger must not carry values it cannot classify.
    pub fn apply(&mut self, clock: &Clock, rows: &[balances::Balance]) -> Result<Applied> {
        self.check_clock(clock)?;
        let mut journal = self.begin(clock);
        let mut applied = Applied::default();
        let mut seen = BTreeSet::new();
        for row in rows {
            if let Some(contract) = &row.contract {
                if let Some(reason) = self.unsupported.get(contract) {
                    return err(format!("row for unsupported contract 0x{}: {reason}", hex_of(contract)));
                }
            }
            if !valid_decimal(&row.amount, false) {
                return err(format!("invalid amount `{}`", row.amount));
            }
            let key = Key {
                contract: row.contract.clone(),
                address: row.address.clone(),
            };
            if !seen.insert(key.clone()) {
                return err("duplicate holder row within block");
            }
            applied.rows += 1;
            applied.zero_rows += usize::from(row.amount == "0");
            let since = self.entries.get(&key).map(|e| e.since).unwrap_or_else(|| {
                applied.new_holders += 1;
                clock.number
            });
            self.set(
                &mut journal,
                key,
                Entry {
                    value: row.amount.clone(),
                    origin: Origin::Observed,
                    since,
                    updated: clock.number,
                },
            );
        }
        journal.rows = applied.rows as u64;
        self.commit(journal);
        Ok(applied)
    }

    /// Apply one block of `evm.balance_state.v1` rows: holder basis values
    /// become entries keyed by market; `GlobalState` rows touch no holder;
    /// `INVALIDATED`/`SUSPENDED` epochs suspend the market; `BOUND` with
    /// `basis_carryover == false` drops the market's retained basis.
    pub fn apply_state(&mut self, clock: &Clock, events: &state::Events) -> Result<Applied> {
        self.check_clock(clock)?;
        if events.clocks.len() != 1 || events.clocks[0].number != clock.number || events.clocks[0].hash != clock.hash {
            return err("balance-state events must carry exactly the applied block clock");
        }
        let mut journal = self.begin(clock);
        let mut applied = Applied::default();
        let mut epochs: Vec<&state::ModelEpoch> = events.epochs.iter().collect();
        epochs.sort_by_key(|e| (e.ordinal, e.kind));
        for epoch in epochs {
            let previous = self.suspended.get(&epoch.market).cloned();
            match state::EpochEventKind::try_from(epoch.kind) {
                Ok(state::EpochEventKind::Invalidated) | Ok(state::EpochEventKind::Suspended) => {
                    self.suspended.insert(
                        epoch.market.clone(),
                        Suspension {
                            epoch: epoch.epoch,
                            reason: epoch.reason,
                            block: clock.number,
                        },
                    );
                }
                Ok(state::EpochEventKind::Bound) => {
                    if !epoch.basis_carryover {
                        let dropped: Vec<Key> = self
                            .entries
                            .keys()
                            .filter(|k| k.contract.as_deref() == Some(epoch.market.as_slice()))
                            .cloned()
                            .collect();
                        for key in dropped {
                            let old = self.entries.remove(&key);
                            if !journal.previous.iter().any(|(k, _)| *k == key) {
                                journal.previous.push((key, old));
                            }
                        }
                    }
                    self.suspended.remove(&epoch.market);
                }
                Ok(state::EpochEventKind::Reaffirmed) => {}
                _ => return err(format!("unknown epoch event kind {}", epoch.kind)),
            }
            if !journal.previous_suspensions.iter().any(|(m, _)| *m == epoch.market) {
                journal.previous_suspensions.push((epoch.market.clone(), previous));
            }
        }
        let mut seen = BTreeSet::new();
        for row in &events.holder_basis {
            if !valid_decimal(&row.value, row.signed) {
                return err(format!("invalid basis value `{}`", row.value));
            }
            let key = Key {
                contract: Some(row.market.clone()),
                address: row.holder.clone(),
            };
            if !seen.insert(key.clone()) {
                return err("duplicate holder basis row within block");
            }
            applied.rows += 1;
            applied.zero_rows += usize::from(row.value == "0");
            let since = self.entries.get(&key).map(|e| e.since).unwrap_or_else(|| {
                applied.new_holders += 1;
                clock.number
            });
            self.set(
                &mut journal,
                key,
                Entry {
                    value: row.value.clone(),
                    origin: Origin::Observed,
                    since,
                    updated: clock.number,
                },
            );
        }
        journal.rows = applied.rows as u64;
        self.commit(journal);
        Ok(applied)
    }

    /// Roll back every block above `to_number`, restoring the exact prior
    /// entries. Fails when the journal no longer covers the range.
    pub fn undo(&mut self, to_number: u64) -> Result<u64> {
        let Some(last) = &self.last else { return err("nothing applied") };
        if to_number >= last.number {
            return err(format!("undo target {to_number} is not below the last block {}", last.number));
        }
        let needed = (last.number - to_number) as usize;
        if self.journal.len() < needed {
            return err(format!("undo depth {} does not cover {needed} blocks", self.journal.len()));
        }
        let mut undone = 0;
        while self.last.as_ref().is_some_and(|l| l.number > to_number) {
            let journal = self.journal.pop_back().expect("journal covers the range");
            for (key, previous) in journal.previous.into_iter().rev() {
                match previous {
                    Some(entry) => self.entries.insert(key, entry),
                    None => self.entries.remove(&key),
                };
            }
            for (market, previous) in journal.previous_suspensions.into_iter().rev() {
                match previous {
                    Some(s) => self.suspended.insert(market, s),
                    None => self.suspended.remove(&market),
                };
            }
            self.emitted_rows -= journal.rows;
            self.blocks_applied -= 1;
            self.undone_blocks += 1;
            undone += 1;
            self.last = self.journal.back().map(|j| j.clock.clone()).or_else(|| {
                Some(Clock {
                    number: journal.clock.number - 1,
                    hash: journal.clock.parent_hash.clone(),
                    parent_hash: Vec::new(),
                })
            });
        }
        Ok(undone)
    }

    pub fn lookup(&self, key: &Key) -> Lookup<'_> {
        if let Some(contract) = &key.contract {
            if let Some(reason) = self.unsupported.get(contract) {
                return Lookup::Unsupported(reason);
            }
            if let Some(s) = self.suspended.get(contract) {
                return Lookup::Suspended {
                    epoch: s.epoch,
                    reason: s.reason,
                };
            }
        }
        match self.entries.get(key) {
            Some(entry) => Lookup::Known(entry),
            None => {
                self.cold_lookups.set(self.cold_lookups.get() + 1);
                Lookup::Unknown
            }
        }
    }
    pub fn entries(&self) -> &BTreeMap<Key, Entry> {
        &self.entries
    }
    pub fn last(&self) -> Option<&Clock> {
        self.last.as_ref()
    }

    /// Compare retained state with reference rows at the last applied block.
    pub fn compare(&self, reference: &[(Key, String)]) -> Comparison {
        let mut c = Comparison {
            reference_rows: reference.len(),
            ..Default::default()
        };
        for (key, value) in reference {
            match self.lookup(key) {
                Lookup::Known(entry) if entry.value == *value => {
                    c.matches += 1;
                    c.known_zero_matches += usize::from(value == "0");
                }
                Lookup::Known(entry) => c.mismatches.push((key.clone(), entry.value.clone(), value.clone())),
                Lookup::Unknown => {
                    c.unknown += 1;
                    c.unknown_nonzero += usize::from(value != "0");
                }
                Lookup::Unsupported(_) => c.unsupported += 1,
                Lookup::Suspended { .. } => c.suspended += 1,
            }
        }
        c
    }

    pub fn report(&self) -> Report {
        let by = |f: &dyn Fn(&Entry) -> bool| self.entries.values().filter(|e| f(e)).count();
        Report {
            blocks_applied: self.blocks_applied,
            first_block: self.first_block,
            last_block: self.last.as_ref().map(|c| c.number),
            last_hash: self.last.as_ref().map(|c| format!("0x{}", hex_of(&c.hash))),
            emitted_rows: self.emitted_rows,
            undone_blocks: self.undone_blocks,
            initialized_holders: self.entries.len(),
            observed_holders: by(&|e| e.origin == Origin::Observed),
            checkpoint_seeded_holders: by(&|e| matches!(e.origin, Origin::Checkpoint { .. })),
            deployment_seeded_holders: by(&|e| matches!(e.origin, Origin::DeploymentZero { .. })),
            known_zero_holders: by(&|e| e.value == "0"),
            cold_unknown_lookups: self.cold_lookups.get(),
            unsupported_contracts: self.unsupported.len(),
            suspended_markets: self.suspended.len(),
            enumerated: self.enumerations.last().cloned(),
        }
    }
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
