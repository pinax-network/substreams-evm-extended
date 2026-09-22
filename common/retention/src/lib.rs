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
//! One ledger retains one output ([`Domain`]): an `evm.balances.v1` amount and
//! an `evm.balance_state.v1` basis of the same token share a [`Key`] but are
//! different quantities. Blocks must be contiguous by number and parent hash;
//! a fork is handled by `undo`, which restores the exact prior state from a
//! bounded journal. The report counts emitted rows, initialized holders by
//! origin, known zeros, cold lookups and undone blocks separately, and never
//! claims a global holder set unless independent enumeration evidence was
//! attached. This is host-side consumer tooling; it is not a map and holds no
//! RPC.
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

/// The output a ledger retains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Domain {
    /// `evm.balances.v1`: unsigned end-of-block amounts keyed by token
    /// (`contract == None` is the native balance). Checkpoints hold
    /// `balanceOf` values.
    Balances,
    /// `evm.balance_state.v1`: holder basis keyed by market, in the basis
    /// units of the market's epoch (scaled balance, shares, signed principal).
    /// Checkpoints hold that basis, not `balanceOf`.
    BalanceState,
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
    Checkpoint { block: u64, hash: Vec<u8>, evidence: String },
    /// EVM-initial zero storage of a token whose creation was observed at
    /// `block`; only valid for holders that cannot precede the creation.
    DeploymentZero { block: u64, hash: Vec<u8> },
    /// An emitted end-of-block row.
    Observed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Exact decimal integer; `"0"` is a known zero. Signed only for
    /// `BASIS_KIND_SIGNED_PRINCIPAL` (Comet principal).
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
/// Identity of the balance-state package instance a ledger follows, bound by
/// its first applied block. Any change is a different stream: start a new
/// ledger from a checkpoint instead of mixing two emission rule sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stream {
    pub chain_id: u64,
    pub package: String,
    pub package_version: String,
    pub spec_revision: u32,
    pub parameters_sha256: String,
}
/// Epoch in force for a market and whether this ledger saw it bound (or
/// reaffirmed as bound). A market first declared by `SUSPENDED`,
/// `INVALIDATED` or rows is not bound, so its first `BOUND` may keep the
/// declared number; every later `BOUND` must advance it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct EpochState {
    epoch: u32,
    bound: bool,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Applied {
    pub rows: usize,
    pub new_holders: usize,
    pub zero_rows: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub domain: Domain,
    pub stream: Option<Stream>,
    pub blocks_applied: u64,
    pub first_block: Option<u64>,
    pub last_block: Option<u64>,
    pub last_hash: Option<String>,
    pub emitted_rows: u64,
    pub undone_blocks: u64,
    /// Holder counts exclude entries of contracts marked unsupported, which
    /// are counted in `unsupported_entries` and never looked up as known.
    pub initialized_holders: usize,
    pub observed_holders: usize,
    pub checkpoint_seeded_holders: usize,
    pub deployment_seeded_holders: usize,
    pub known_zero_holders: usize,
    pub unsupported_entries: usize,
    pub cold_unknown_lookups: u64,
    pub unsupported_contracts: usize,
    pub suspended_markets: usize,
    /// Only what `attach_enumeration` supplied as independent evidence, each
    /// at its own block; the retained set is never reported as the global
    /// holder set.
    pub enumerated: Vec<Enumeration>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Enumeration {
    pub contract: Vec<u8>,
    pub holders: usize,
    /// The applied block the enumeration describes; `retained_missing` is
    /// exact for that block. Undoing the block discards the enumeration.
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
    /// Unknown rows are coverage gaps, never matches and never failures. An
    /// empty reference establishes nothing.
    pub fn status(&self) -> &'static str {
        if self.reference_rows == 0 {
            "no_reference"
        } else if !self.mismatches.is_empty() {
            "mismatch"
        } else if self.unknown > 0 || self.unsupported > 0 || self.suspended > 0 {
            "coverage_gap"
        } else {
            "bounded_parity"
        }
    }
}

const UINT256_MAX: &str = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
const INT256_MAX: &str = "57896044618658097711785492504343953926634992332820282019728792003956564819967";
const INT256_MIN_MAGNITUDE: &str = "57896044618658097711785492504343953926634992332820282019728792003956564819968";

fn not_above(digits: &str, max: &str) -> bool {
    digits.len() < max.len() || (digits.len() == max.len() && digits <= max)
}

/// Canonical exact decimal: ASCII digits, no leading zeros, a sign only when
/// `signed` (never `-0`), within uint256 or int256.
fn valid_decimal(s: &str, signed: bool) -> bool {
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) if signed => (true, rest),
        _ => (false, s),
    };
    let canonical = !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) && (digits == "0" || !digits.starts_with('0'));
    canonical
        && match (signed, negative) {
            (true, true) => digits != "0" && not_above(digits, INT256_MIN_MAGNITUDE),
            (true, false) => not_above(digits, INT256_MAX),
            (false, _) => not_above(digits, UINT256_MAX),
        }
}

/// A named value of a proto enum: known to this build and not `UNSPECIFIED`
/// (every enum in the contract reserves 0 for it).
fn named<E: TryFrom<i32>>(value: i32) -> bool {
    value != 0 && E::try_from(value).is_ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Journal {
    clock: Clock,
    previous_clock: Option<Clock>,
    previous: Vec<(Key, Option<Entry>)>,
    previous_suspensions: Vec<(Vec<u8>, Option<Suspension>)>,
    previous_epochs: Vec<(Vec<u8>, Option<EpochState>)>,
    new_contracts: Vec<Vec<u8>>,
    previous_stream: Option<Stream>,
    rows: u64,
}

/// Retained holder state with a bounded undo journal.
#[derive(Debug, Clone)]
pub struct Ledger {
    domain: Domain,
    entries: BTreeMap<Key, Entry>,
    unsupported: BTreeMap<Vec<u8>, String>,
    suspended: BTreeMap<Vec<u8>, Suspension>,
    /// Epoch in force per market (`evm.balance_state.v1` only).
    epochs: BTreeMap<Vec<u8>, EpochState>,
    /// First block at which the ledger held any state for a contract: a
    /// checkpoint, a row, a seed or an epoch event. Dropping entries later
    /// does not make a token new again.
    contracts: BTreeMap<Vec<u8>, u64>,
    stream: Option<Stream>,
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
    pub fn new(undo_depth: usize, domain: Domain) -> Self {
        Self {
            domain,
            entries: BTreeMap::new(),
            unsupported: BTreeMap::new(),
            suspended: BTreeMap::new(),
            epochs: BTreeMap::new(),
            contracts: BTreeMap::new(),
            stream: None,
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

    pub fn domain(&self) -> Domain {
        self.domain
    }

    fn require_domain(&self, domain: Domain) -> Result<()> {
        if self.domain != domain {
            return err(format!("a {:?} ledger does not retain {domain:?} rows; use one ledger per output", self.domain));
        }
        Ok(())
    }

    /// Seed a verified value at `(block, hash)`; every seed must identify the
    /// same parent of the first applied block, which `apply` enforces. A
    /// `Balances` seed is an unsigned `balanceOf`; a `BalanceState` seed is
    /// the market's basis and may be signed only for a signed-principal basis.
    pub fn seed_checkpoint(&mut self, key: Key, value: &str, block: u64, hash: &[u8], evidence: &str) -> Result<()> {
        if self.last.is_some() {
            return err("checkpoints are seeded before the first block");
        }
        if self.domain == Domain::BalanceState && key.contract.is_none() {
            return err("a balance-state checkpoint names its market");
        }
        if !valid_decimal(value, self.domain == Domain::BalanceState) {
            return err(format!("invalid checkpoint value `{value}`"));
        }
        if evidence.is_empty() {
            return err("checkpoint evidence reference required");
        }
        if hash.len() != 32 {
            return err("checkpoint identity must have a 32-byte hash");
        }
        if let Some(reason) = key.contract.as_ref().and_then(|c| self.unsupported.get(c)) {
            return err(format!("checkpoint for unsupported contract: {reason}"));
        }
        if self.entries.contains_key(&key) {
            return err("holder already seeded");
        }
        for entry in self.entries.values() {
            if let Origin::Checkpoint {
                block: seeded_block,
                hash: seeded_hash,
                ..
            } = &entry.origin
            {
                if *seeded_block != block || seeded_hash != hash {
                    return err("all checkpoint seeds must identify the same block and hash");
                }
            }
        }
        if let Some(contract) = &key.contract {
            self.contracts.entry(contract.clone()).or_insert(block);
        }
        self.entries.insert(
            key,
            Entry {
                value: value.to_string(),
                origin: Origin::Checkpoint {
                    block,
                    hash: hash.to_vec(),
                    evidence: evidence.to_string(),
                },
                since: block,
                updated: block,
            },
        );
        Ok(())
    }

    /// Seed EVM-initial zeros immediately after applying the exact creation
    /// block, for holders that cannot hold the token before creation. The
    /// caller must qualify the creation; this ledger binds its identity and
    /// adds the seeds to that block's undo journal. Refused for a token the
    /// ledger held any state for before the creation block. A holder the
    /// creation block already wrote (a constructor mint) keeps its row.
    /// Returns the number of holders seeded.
    pub fn seed_deployment_zero(&mut self, contract: &[u8], holders: &BTreeSet<Vec<u8>>, creation: &Clock) -> Result<usize> {
        if let Some(reason) = self.unsupported.get(contract) {
            return err(format!("deployment seed for unsupported contract 0x{}: {reason}", hex_of(contract)));
        }
        if self.last.as_ref() != Some(creation) {
            return err("deployment identity must match the latest applied block");
        }
        if self.journal.back().is_none_or(|journal| journal.clock != *creation) {
            return err("deployment initialization requires the creation block's undo journal");
        }
        if self.contracts.get(contract).is_some_and(|first| *first != creation.number) {
            return err("deployment token already has holder state before its creation block");
        }
        if self
            .entries
            .iter()
            .any(|(k, e)| k.contract.as_deref() == Some(contract) && matches!(e.origin, Origin::DeploymentZero { .. }))
        {
            return err("deployment zeros already seeded for this token");
        }
        let journal = self.journal.back_mut().expect("creation journal checked above");
        let mut seeded = 0;
        for holder in holders {
            let key = Key {
                contract: Some(contract.to_vec()),
                address: holder.clone(),
            };
            if self.entries.contains_key(&key) {
                continue;
            }
            self.entries.insert(
                key.clone(),
                Entry {
                    value: "0".into(),
                    origin: Origin::DeploymentZero {
                        block: creation.number,
                        hash: creation.hash.clone(),
                    },
                    since: creation.number,
                    updated: creation.number,
                },
            );
            // Keep an earlier change of this block (such as a row applied
            // before a BOUND dropped it): undo restores the pre-block value.
            if !journal.previous.iter().any(|(k, _)| *k == key) {
                journal.previous.push((key, None));
            }
            seeded += 1;
        }
        if !self.contracts.contains_key(contract) {
            self.contracts.insert(contract.to_vec(), creation.number);
            journal.new_contracts.push(contract.to_vec());
        }
        Ok(seeded)
    }

    pub fn mark_unsupported(&mut self, contract: &[u8], reason: &str) {
        self.unsupported.insert(contract.to_vec(), reason.to_string());
    }

    /// Independent enumeration evidence for one contract at the latest
    /// applied block. The report carries every attachment verbatim next to
    /// the retained counts.
    pub fn attach_enumeration(&mut self, contract: &[u8], holders: &BTreeSet<Vec<u8>>, block: u64, evidence: &str) -> Result<()> {
        if evidence.is_empty() || holders.is_empty() {
            return err("enumeration needs holders and an evidence reference");
        }
        if self.last.as_ref().map(|c| c.number) != Some(block) {
            return err("an enumeration describes the latest applied block");
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
                let Some(next) = last.number.checked_add(1) else {
                    return err("block number has no representable successor");
                };
                if clock.number != next {
                    return err(format!("block gap: expected {next}, got {}", clock.number));
                }
                if clock.parent_hash != last.hash {
                    return err(format!("fork at block {}: parent does not match retained hash; undo first", clock.number));
                }
            }
            None => {
                for (key, entry) in &self.entries {
                    if let Origin::Checkpoint { block, hash, .. } = &entry.origin {
                        if block.checked_add(1) != Some(clock.number) || hash != &clock.parent_hash {
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

    fn begin(&self, clock: &Clock) -> Journal {
        Journal {
            clock: clock.clone(),
            previous_clock: self.last.clone(),
            previous: Vec::new(),
            previous_suspensions: Vec::new(),
            previous_epochs: Vec::new(),
            new_contracts: Vec::new(),
            previous_stream: self.stream.clone(),
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
    fn touch(&mut self, journal: &mut Journal, contract: &[u8]) {
        if !self.contracts.contains_key(contract) {
            self.contracts.insert(contract.to_vec(), journal.clock.number);
            journal.new_contracts.push(contract.to_vec());
        }
    }
    fn set_epoch(&mut self, journal: &mut Journal, market: &[u8], epoch: EpochState) {
        let previous = self.epochs.insert(market.to_vec(), epoch);
        if !journal.previous_epochs.iter().any(|(m, _)| m == market) {
            journal.previous_epochs.push((market.to_vec(), previous));
        }
    }
    fn observe(&mut self, journal: &mut Journal, applied: &mut Applied, contract: Option<&[u8]>, address: &[u8], value: &str) {
        if let Some(contract) = contract {
            self.touch(journal, contract);
        }
        let key = Key {
            contract: contract.map(<[u8]>::to_vec),
            address: address.to_vec(),
        };
        let clock = journal.clock.number;
        applied.rows += 1;
        applied.zero_rows += usize::from(value == "0");
        let since = self.entries.get(&key).map(|e| e.since).unwrap_or_else(|| {
            applied.new_holders += 1;
            clock
        });
        self.set(
            journal,
            key,
            Entry {
                value: value.to_string(),
                origin: Origin::Observed,
                since,
                updated: clock,
            },
        );
    }

    /// Apply one block of `evm.balances.v1` rows. Rows for unsupported
    /// contracts fail: the ledger must not carry values it cannot classify.
    pub fn apply(&mut self, clock: &Clock, rows: &[balances::Balance]) -> Result<Applied> {
        self.require_domain(Domain::Balances)?;
        self.check_clock(clock)?;
        // Validate the complete block before changing entries or journal state.
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
            if !seen.insert((row.contract.as_deref(), row.address.as_slice())) {
                return err("duplicate holder row within block");
            }
        }
        let mut journal = self.begin(clock);
        let mut applied = Applied::default();
        for row in rows {
            self.observe(&mut journal, &mut applied, row.contract.as_deref(), &row.address, &row.amount);
        }
        journal.rows = applied.rows as u64;
        self.commit(journal);
        Ok(applied)
    }

    /// Apply one block of `evm.balance_state.v1` rows.
    ///
    /// The block must be complete (its `BlockClock` counts equal the rows)
    /// and from the same package instance as earlier blocks. Epoch rows are
    /// applied in `(ordinal, kind)` order: `INVALIDATED`/`SUSPENDED` suspend
    /// the market, `BOUND` resumes it with a strictly newer epoch and, with
    /// `basis_carryover == false`, drops the market's retained basis. Holder
    /// rows of an older epoch in the same block (effects before the
    /// successor's activation ordinal) are applied just before that `BOUND`,
    /// so they are carried over or dropped with the rest of the basis.
    /// `GlobalState` rows are validated and touch no holder entry.
    pub fn apply_state(&mut self, clock: &Clock, events: &state::Events) -> Result<Applied> {
        self.require_domain(Domain::BalanceState)?;
        self.check_clock(clock)?;
        let [block_clock] = events.clocks.as_slice() else {
            return err("balance-state events must carry exactly the applied block clock");
        };
        if block_clock.number != clock.number || block_clock.hash != clock.hash || block_clock.parent_hash != clock.parent_hash {
            return err("balance-state events must carry exactly the applied block clock");
        }
        for (table, declared, landed) in [
            ("epoch", block_clock.epoch_count, events.epochs.len()),
            ("dependency", block_clock.dependency_count, events.dependencies.len()),
            ("alias", block_clock.alias_count, events.aliases.len()),
            ("holder basis", block_clock.holder_basis_count, events.holder_basis.len()),
            ("global state", block_clock.global_state_count, events.global_state.len()),
        ] {
            if declared as usize != landed {
                return err(format!(
                    "partial block {}: the clock declares {declared} {table} rows but {landed} are present",
                    clock.number
                ));
            }
        }
        let stream = Stream {
            chain_id: block_clock.chain_id,
            package: block_clock.package.clone(),
            package_version: block_clock.package_version.clone(),
            spec_revision: block_clock.spec_revision,
            parameters_sha256: block_clock.parameters_sha256.clone(),
        };
        if stream.chain_id == 0 || stream.package.is_empty() || stream.package_version.is_empty() {
            return err("the block clock must name its chain and package");
        }
        if stream.parameters_sha256.len() != 64 || !stream.parameters_sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
            return err("the block clock must carry the parameters SHA-256");
        }
        if let Some(bound) = &self.stream {
            if *bound != stream {
                return err(format!(
                    "block {} comes from a different stream ({stream:?}, ledger follows {bound:?}); start a new ledger from a checkpoint",
                    clock.number
                ));
            }
        }
        let chain = stream.chain_id;

        // Epoch rows: validated kinds, a deterministic order, and an epoch
        // sequence that only advances by BOUND.
        let mut epochs = Vec::with_capacity(events.epochs.len());
        for epoch in &events.epochs {
            let kind = match state::EpochEventKind::try_from(epoch.kind) {
                Ok(kind) if kind != state::EpochEventKind::Unspecified => kind,
                _ => return err(format!("unknown epoch event kind {}", epoch.kind)),
            };
            if epoch.epoch == 0 || epoch.chain_id != chain {
                return err(format!("epoch row for market 0x{} has epoch 0 or another chain", hex_of(&epoch.market)));
            }
            epochs.push((epoch, kind));
        }
        epochs.sort_by(|(a, _), (b, _)| (a.ordinal, a.kind, &a.market, a.epoch).cmp(&(b.ordinal, b.kind, &b.market, b.epoch)));
        let mut planned: BTreeMap<&[u8], EpochState> = BTreeMap::new();
        for (epoch, kind) in &epochs {
            let market = epoch.market.as_slice();
            let next = match (kind, planned.get(market).or_else(|| self.epochs.get(market)).copied()) {
                (state::EpochEventKind::Bound, Some(c)) if epoch.epoch < c.epoch || (epoch.epoch == c.epoch && c.bound) => {
                    return err(format!(
                        "BOUND epoch {} of market 0x{} does not advance its epoch {}",
                        epoch.epoch,
                        hex_of(market),
                        c.epoch
                    ))
                }
                (state::EpochEventKind::Bound, _) => EpochState {
                    epoch: epoch.epoch,
                    bound: true,
                },
                (_, Some(c)) if epoch.epoch != c.epoch => {
                    return err(format!(
                        "{kind:?} row for epoch {} of market 0x{} while epoch {} is in force",
                        epoch.epoch,
                        hex_of(market),
                        c.epoch
                    ))
                }
                (_, Some(c)) => c,
                (_, None) => EpochState {
                    epoch: epoch.epoch,
                    bound: *kind == state::EpochEventKind::Reaffirmed,
                },
            };
            planned.insert(market, next);
        }
        // Holder and global rows must belong to an epoch of this block: the
        // one in force at its end, or an earlier one a BOUND of this block
        // supersedes. A market with no known epoch adopts the rows' epoch.
        let mut adopted: BTreeMap<Vec<u8>, u32> = BTreeMap::new();
        let mut member = |market: &[u8], epoch: u32| -> Result<()> {
            let before = self.epochs.get(market).map(|s| s.epoch);
            let Some(after) = planned.get(market).map(|s| s.epoch).or(before) else {
                return match adopted.insert(market.to_vec(), epoch) {
                    Some(other) if other != epoch => err(format!("rows of market 0x{} carry epochs {other} and {epoch} without a BOUND", hex_of(market))),
                    _ => Ok(()),
                };
            };
            let superseded = epoch < after
                && before.is_none_or(|b| epoch >= b)
                && epochs
                    .iter()
                    .any(|(e, k)| *k == state::EpochEventKind::Bound && e.market == market && e.epoch > epoch);
            if epoch == after || superseded {
                Ok(())
            } else {
                err(format!(
                    "row for epoch {epoch} of market 0x{} outside the epochs of block {}",
                    hex_of(market),
                    clock.number
                ))
            }
        };
        let mut seen = BTreeSet::new();
        for row in &events.holder_basis {
            if let Some(reason) = self.unsupported.get(&row.market) {
                return err(format!("row for unsupported market 0x{}: {reason}", hex_of(&row.market)));
            }
            if row.epoch == 0 || row.chain_id != chain {
                return err("holder basis row has epoch 0 or another chain");
            }
            let signed_kind = row.basis_kind == state::BasisKind::SignedPrincipal as i32;
            if !named::<state::BasisKind>(row.basis_kind)
                || !named::<state::Observation>(row.observation)
                || !named::<state::Scope>(row.scope)
                || row.boundary != state::Boundary::EndOfBlock as i32
                || row.signed != signed_kind
            {
                return err(
                    "holder basis row has an unknown kind, observation or scope, a boundary other than END_OF_BLOCK, or a sign that does not match its kind",
                );
            }
            if !valid_decimal(&row.value, row.signed) {
                return err(format!("invalid basis value `{}`", row.value));
            }
            if !seen.insert((row.market.as_slice(), row.epoch, row.holder.as_slice())) {
                return err("duplicate holder basis row within block");
            }
            member(&row.market, row.epoch)?;
        }
        for row in &events.global_state {
            if let Some(reason) = self.unsupported.get(&row.market) {
                return err(format!("row for unsupported market 0x{}: {reason}", hex_of(&row.market)));
            }
            if row.epoch == 0 || row.chain_id != chain {
                return err("global state row has epoch 0 or another chain");
            }
            if !named::<state::StateField>(row.field)
                || !named::<state::Observation>(row.observation)
                || !named::<state::Boundary>(row.boundary)
                || !named::<state::Scope>(row.scope)
            {
                return err("global state row has an unknown field, observation, boundary or scope");
            }
            if !valid_decimal(&row.value, row.signed) {
                return err(format!("invalid global state value `{}`", row.value));
            }
            member(&row.market, row.epoch)?;
        }

        let mut journal = self.begin(clock);
        self.stream = Some(stream);
        let mut applied = Applied::default();
        let mut rows: Vec<&state::HolderBasis> = events.holder_basis.iter().collect();
        rows.sort_by(|a, b| (&a.market, a.epoch, &a.holder).cmp(&(&b.market, b.epoch, &b.holder)));
        let mut done = vec![false; rows.len()];
        for (epoch, kind) in &epochs {
            self.touch(&mut journal, &epoch.market);
            let previous = self.suspended.get(&epoch.market).cloned();
            match kind {
                state::EpochEventKind::Invalidated | state::EpochEventKind::Suspended => {
                    self.suspended.insert(
                        epoch.market.clone(),
                        Suspension {
                            epoch: epoch.epoch,
                            reason: epoch.reason,
                            block: clock.number,
                        },
                    );
                }
                state::EpochEventKind::Bound => {
                    // Effects of an earlier epoch in this block precede the
                    // successor's activation.
                    for (row, done) in rows.iter().zip(done.iter_mut()) {
                        if !*done && row.market == epoch.market && row.epoch < epoch.epoch {
                            self.observe(&mut journal, &mut applied, Some(&row.market), &row.holder, &row.value);
                            *done = true;
                        }
                    }
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
                state::EpochEventKind::Reaffirmed => {}
                state::EpochEventKind::Unspecified => unreachable!("epoch kinds validated before mutation"),
            }
            if !journal.previous_suspensions.iter().any(|(m, _)| *m == epoch.market) {
                journal.previous_suspensions.push((epoch.market.clone(), previous));
            }
            let next = match kind {
                state::EpochEventKind::Bound => EpochState {
                    epoch: epoch.epoch,
                    bound: true,
                },
                _ => self.epochs.get(&epoch.market).copied().unwrap_or(EpochState {
                    epoch: epoch.epoch,
                    bound: *kind == state::EpochEventKind::Reaffirmed,
                }),
            };
            self.set_epoch(&mut journal, &epoch.market, next);
        }
        for (row, done) in rows.iter().zip(done) {
            if !done {
                self.observe(&mut journal, &mut applied, Some(&row.market), &row.holder, &row.value);
            }
        }
        for (market, epoch) in adopted {
            self.set_epoch(&mut journal, &market, EpochState { epoch, bound: false });
        }
        for row in &events.global_state {
            self.touch(&mut journal, &row.market);
        }
        journal.rows = applied.rows as u64;
        self.commit(journal);
        Ok(applied)
    }

    /// Roll back every block above `to_number`, restoring the exact prior
    /// state. Fails when the journal no longer covers the range.
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
            for (market, previous) in journal.previous_epochs.into_iter().rev() {
                match previous {
                    Some(epoch) => self.epochs.insert(market, epoch),
                    None => self.epochs.remove(&market),
                };
            }
            for contract in journal.new_contracts {
                self.contracts.remove(&contract);
            }
            self.stream = journal.previous_stream;
            self.emitted_rows -= journal.rows;
            self.blocks_applied -= 1;
            self.undone_blocks += 1;
            undone += 1;
            self.last = journal.previous_clock.or_else(|| {
                Some(Clock {
                    number: journal.clock.number - 1,
                    hash: journal.clock.parent_hash.clone(),
                    parent_hash: Vec::new(),
                })
            });
        }
        if self.blocks_applied == 0 {
            self.first_block = None;
        }
        self.enumerations.retain(|e| e.block <= to_number);
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
    /// Epoch in force for a market, as retained from its epoch or row events.
    pub fn epoch(&self, market: &[u8]) -> Option<u32> {
        self.epochs.get(market).map(|s| s.epoch)
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
        let supported = |k: &Key| k.contract.as_ref().is_none_or(|c| !self.unsupported.contains_key(c));
        let by = |f: &dyn Fn(&Entry) -> bool| self.entries.iter().filter(|(k, e)| supported(k) && f(e)).count();
        Report {
            domain: self.domain,
            stream: self.stream.clone(),
            blocks_applied: self.blocks_applied,
            first_block: self.first_block,
            last_block: self.last.as_ref().map(|c| c.number),
            last_hash: self.last.as_ref().map(|c| format!("0x{}", hex_of(&c.hash))),
            emitted_rows: self.emitted_rows,
            undone_blocks: self.undone_blocks,
            initialized_holders: by(&|_| true),
            observed_holders: by(&|e| e.origin == Origin::Observed),
            checkpoint_seeded_holders: by(&|e| matches!(e.origin, Origin::Checkpoint { .. })),
            deployment_seeded_holders: by(&|e| matches!(e.origin, Origin::DeploymentZero { .. })),
            known_zero_holders: by(&|e| e.value == "0"),
            unsupported_entries: self.entries.keys().filter(|k| !supported(k)).count(),
            cold_unknown_lookups: self.cold_lookups.get(),
            unsupported_contracts: self.unsupported.len(),
            suspended_markets: self.suspended.len(),
            enumerated: self.enumerations.clone(),
        }
    }
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
