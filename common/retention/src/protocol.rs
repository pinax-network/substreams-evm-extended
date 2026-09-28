//! Bounded host reference state, not a production sink. Full snapshots in the
//! undo journal deliberately favor auditable rollback over sink-scale memory
//! efficiency. At most `undo_depth` snapshots are retained. The existing
//! holder ledger still owns stream continuity, row counts and suspension.
//!
//! Original protobuf rows are preserved, including observation, scale and
//! slot/log provenance. `effective_epoch` records an explicitly permitted
//! carryover without rewriting the original row's epoch. Log evidence never
//! substitutes for a stored input; derived rows expire at the next block.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Fact<T> {
    pub row: T,
    pub origin: Origin,
    pub observed_at: state::BlockClock,
    pub effective_epoch: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalKey {
    pub market: Vec<u8>,
    pub field: i32,
    pub key: Vec<u8>,
    pub observation: i32,
}
impl GlobalKey {
    pub fn new(market: &[u8], field: state::StateField, key: &[u8], observation: state::Observation) -> Self {
        Self {
            market: market.to_vec(),
            field: field as i32,
            key: key.to_vec(),
            observation: observation as i32,
        }
    }
    fn of(row: &state::GlobalState) -> Self {
        Self {
            market: row.market.clone(),
            field: row.field,
            key: row.key.clone(),
            observation: row.observation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
    Missing,
    Unbound,
    Unsupported(String),
    Suspended { epoch: u32, reason: i32 },
}

#[derive(Clone, Debug, Default, PartialEq)]
struct State {
    clock: Option<state::BlockClock>,
    models: BTreeMap<Vec<u8>, Fact<state::ModelEpoch>>,
    holders: BTreeMap<Key, Fact<state::HolderBasis>>,
    globals: BTreeMap<GlobalKey, Fact<state::GlobalState>>,
    dependencies: BTreeMap<Vec<u8>, Vec<state::Dependency>>,
}

#[derive(Default)]
struct Traversal {
    holders_done: BTreeSet<usize>,
    globals_done: BTreeSet<usize>,
    interrupted: BTreeSet<Vec<u8>>,
}

/// An independently verified snapshot of explicitly listed inputs at one
/// exact canonical block. Missing holders/globals remain unknown. The clock
/// binds chain/package/version/spec/parameters; counts describe these rows.
/// BOUND declarations identify the active models (activation may be earlier).
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub events: state::Events,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProtocolReport {
    /// Primitive basis observations (including unbound/suspended ones), not
    /// a count of evaluable protocol amounts. See active_model_basis_holders;
    /// even those can still lack required global inputs or qualification.
    pub raw_basis_ledger: Report,
    pub active_model_basis_holders: usize,
    pub bound_models: usize,
    pub stored_globals: usize,
    pub qualified_constants: usize,
    pub log_observations: usize,
    pub current_block_derived: usize,
    pub timestamp: Option<u64>,
    pub undo_snapshots: usize,
}

#[derive(Clone, Debug)]
pub struct ProtocolLedger {
    ledger: Ledger,
    state: State,
    history: VecDeque<State>,
    undo_depth: usize,
}

fn identity(clock: &state::BlockClock) -> Clock {
    Clock {
        number: clock.number,
        hash: clock.hash.clone(),
        parent_hash: clock.parent_hash.clone(),
    }
}

/// Compare model identity, ignoring only lifecycle/evidence fields of the
/// announcement. Formula, runtime, activation, scales and carryover flags
/// remain part of the binding.
pub fn same_model(a: &state::ModelEpoch, b: &state::ModelEpoch) -> bool {
    fn binding(mut row: state::ModelEpoch) -> state::ModelEpoch {
        row.kind = 0;
        row.reason = 0;
        row.scope = 0;
        row.ordinal = 0;
        row.transaction_index = 0;
        row.transaction_hash.clear();
        row.call_index = 0;
        row.evidence_contract.clear();
        row.evidence_slot.clear();
        row.evidence_previous_word.clear();
        row.evidence_word.clear();
        row.evidence_code_hash.clear();
        row
    }
    binding(a.clone()) == binding(b.clone())
}

impl ProtocolLedger {
    pub fn new(undo_depth: usize) -> Self {
        Self {
            ledger: Ledger::new(undo_depth, Domain::BalanceState),
            state: State::default(),
            history: VecDeque::new(),
            undo_depth,
        }
    }

    pub fn from_checkpoint(undo_depth: usize, checkpoint: Checkpoint) -> Result<Self> {
        if checkpoint.evidence.trim().is_empty() {
            return err("protocol checkpoint requires independent evidence");
        }
        if checkpoint.events.epochs.iter().any(|e| e.kind != state::EpochEventKind::Bound as i32) {
            return err("checkpoint models must explicitly identify their active BOUND epochs");
        }
        let mut result = Self::new(undo_depth);
        result.apply_inner(&checkpoint.events, Some(&checkpoint.evidence))?;
        // The checkpoint is the origin, not an emitted/applied stream block.
        result.ledger.blocks_applied = 0;
        result.ledger.first_block = None;
        result.ledger.emitted_rows = 0;
        result.ledger.journal.clear();
        result.history.clear();
        for entry in result.ledger.entries.values_mut() {
            let clock = result.state.clock.as_ref().expect("validated clock");
            entry.origin = Origin::Checkpoint {
                block: clock.number,
                hash: clock.hash.clone(),
                evidence: checkpoint.evidence.clone(),
            };
        }
        Ok(result)
    }

    /// Validate and stage the complete block. A refusal changes neither
    /// inputs, counts, identities nor either undo journal.
    pub fn apply(&mut self, events: &state::Events) -> Result<Applied> {
        let mut staged = self.clone();
        let applied = staged.apply_inner(events, None)?;
        *self = staged;
        Ok(applied)
    }

    fn apply_inner(&mut self, events: &state::Events, checkpoint: Option<&str>) -> Result<Applied> {
        let [clock] = events.clocks.as_slice() else {
            return err("protocol block requires one clock");
        };
        if !events.aliases.is_empty() {
            return err("protocol reference ledger does not evaluate asset aliases");
        }
        if clock.state_root.len() != 32 || !matches!(clock.producer_version, 4 | 5) || clock.spec_revision == 0 {
            return err("protocol clock requires a state root, qualified producer version and spec revision");
        }
        if self.state.clock.as_ref().is_some_and(|last| clock.timestamp < last.timestamp) {
            return err("canonical protocol timestamp moved backwards");
        }
        let at = identity(clock);
        let origin = checkpoint.map_or(Origin::Observed, |evidence| Origin::Checkpoint {
            block: at.number,
            hash: at.hash.clone(),
            evidence: evidence.into(),
        });
        let before = self.state.clone();
        let mut traversal = Traversal {
            interrupted: self.ledger.suspended.keys().cloned().collect(),
            ..Default::default()
        };
        let input = if checkpoint.is_some() { Input::Checkpoint } else { Input::Emitted };
        let applied = self.ledger.apply_state_input(&at, events, input)?;
        self.state.globals.retain(|key, _| key.observation != state::Observation::Derived as i32);

        let mut epochs: Vec<_> = events.epochs.iter().collect();
        epochs.sort_by_key(|e| (e.ordinal, e.kind, &e.market, e.epoch));
        for epoch in epochs {
            match state::EpochEventKind::try_from(epoch.kind).expect("ledger validated kind") {
                state::EpochEventKind::Bound => {
                    self.ingest_rows(
                        events,
                        clock,
                        &origin,
                        |market, number| market == epoch.market && number < epoch.epoch,
                        &mut traversal,
                    )?;
                    validate_model(epoch, clock, checkpoint.is_some())?;
                    let previous_model = self.state.models.get(&epoch.market);
                    let gap = traversal.interrupted.contains(&epoch.market) || previous_model.is_none();
                    if epoch.basis_carryover
                        && !gap
                        && previous_model.is_some_and(|previous| {
                            let old = &previous.row;
                            old.basis_kind != epoch.basis_kind
                                || old.basis_signed != epoch.basis_signed
                                || old.basis_bit_width != epoch.basis_bit_width
                                || old.basis_bit_offset != epoch.basis_bit_offset
                        })
                    {
                        return err("basis carryover changes the holder storage interpretation");
                    }
                    self.state.holders.retain(|key, fact| {
                        if key.contract.as_deref() != Some(epoch.market.as_slice()) {
                            return true;
                        }
                        if !epoch.basis_carryover || gap {
                            return false;
                        }
                        fact.effective_epoch = epoch.epoch;
                        true
                    });
                    self.state.globals.retain(|key, fact| {
                        if key.market != epoch.market {
                            return true;
                        }
                        // Constants bind their original runtime; they must
                        // be explicitly redeclared even across storage carryover.
                        if !epoch.global_carryover || gap || key.observation != state::Observation::ObservedWrite as i32 {
                            return false;
                        }
                        fact.effective_epoch = epoch.epoch;
                        true
                    });
                    self.state.dependencies.remove(&epoch.market);
                    self.state.models.insert(
                        epoch.market.clone(),
                        Fact {
                            row: epoch.clone(),
                            origin: origin.clone(),
                            observed_at: clock.clone(),
                            effective_epoch: epoch.epoch,
                        },
                    );
                    traversal.interrupted.remove(&epoch.market);
                }
                state::EpochEventKind::Reaffirmed => {
                    validate_model(epoch, clock, true)?;
                    if let Some(bound) = self.state.models.get(&epoch.market) {
                        if !same_model(&bound.row, epoch) {
                            return err("heartbeat changed the qualified model binding");
                        }
                    }
                    // An isolated heartbeat never proves uninterrupted
                    // history since activation and does not bind a model.
                }
                state::EpochEventKind::Invalidated | state::EpochEventKind::Suspended => {
                    traversal.interrupted.insert(epoch.market.clone());
                }
                state::EpochEventKind::Unspecified => unreachable!(),
            }
        }
        self.ingest_rows(events, clock, &origin, |_, _| true, &mut traversal)?;
        // Respect holder-ledger drops, including its post-suspension gap rule.
        self.state.holders.retain(|key, _| self.ledger.entries.contains_key(key));
        for row in &events.dependencies {
            if row.chain_id != clock.chain_id
                || !named::<state::DependencyRole>(row.role)
                || !named::<state::BindingKind>(row.binding)
                || !matches!(
                    state::EpochEventKind::try_from(row.kind),
                    Ok(state::EpochEventKind::Bound | state::EpochEventKind::Reaffirmed)
                )
                || row.contract.len() != 20
                || row.market.len() != 20
                || (!row.parent.is_empty() && row.parent.len() != 20)
                || row.depth == 0
                || row.source_pin.is_empty()
            {
                return err("invalid protocol dependency binding");
            }
            if !events
                .epochs
                .iter()
                .any(|e| e.market == row.market && e.epoch == row.epoch && e.kind == row.kind)
            {
                return err("dependency requires its matching model declaration");
            }
            if (row.binding == state::BindingKind::CodeHash as i32 && row.code_hash.len() != 32)
                || (!row.code_hash.is_empty() && row.code_hash.len() != 32)
                || (row.binding == state::BindingKind::StoragePointer as i32
                    && (row.pointer_contract.len() != 20 || row.pointer_slot.len() != 32 || row.pointer_value.len() != 32))
            {
                return err("dependency has malformed runtime or pointer identity");
            }
            if self.ledger.epoch(&row.market) != Some(row.epoch) {
                continue;
            }
            let deps = self.state.dependencies.entry(row.market.clone()).or_default();
            if let Some(previous) = deps.iter_mut().find(|d| d.role == row.role && d.contract == row.contract) {
                let mut normalized = row.clone();
                normalized.kind = previous.kind;
                if *previous != normalized {
                    return err("dependency binding changed without a new epoch");
                }
            } else {
                deps.push(row.clone());
            }
        }
        for epoch in events.epochs.iter().filter(|e| e.kind == state::EpochEventKind::Reaffirmed as i32) {
            if before.models.get(&epoch.market).is_some_and(|model| model.effective_epoch == epoch.epoch) {
                let previous = before.dependencies.get(&epoch.market).cloned().unwrap_or_default();
                let declared: Vec<_> = events
                    .dependencies
                    .iter()
                    .filter(|d| d.market == epoch.market && d.epoch == epoch.epoch && d.kind == epoch.kind)
                    .cloned()
                    .collect();
                if dependency_set(previous) != dependency_set(declared) {
                    return err("heartbeat changed or omitted the bound dependency set");
                }
            }
        }
        self.state.clock = Some(clock.clone());
        self.history.push_back(before);
        while self.history.len() > self.undo_depth {
            self.history.pop_front();
        }
        Ok(applied)
    }

    fn ingest_rows(
        &mut self,
        events: &state::Events,
        at: &state::BlockClock,
        origin: &Origin,
        select: impl Fn(&[u8], u32) -> bool,
        traversal: &mut Traversal,
    ) -> Result<()> {
        for (i, row) in events.holder_basis.iter().enumerate() {
            if traversal.holders_done.contains(&i) || !select(&row.market, row.epoch) {
                continue;
            }
            if row.market.len() != 20 || row.holder.len() != 20 {
                return err("invalid protocol holder identity");
            }
            if row.observation != state::Observation::ObservedWrite as i32 {
                return err("holder basis must be an observed storage input");
            }
            if row.storage_contract.len() != 20
                || row.storage_slot.len() != 32
                || (!row.raw_word.is_empty() && row.raw_word.len() != 32)
                || (!row.raw_previous_word.is_empty() && row.raw_previous_word.len() != 32)
            {
                return err("holder basis has malformed storage provenance");
            }
            if let Some(model) = self.state.models.get(&row.market).filter(|model| model.effective_epoch == row.epoch) {
                if row.basis_kind != model.row.basis_kind
                    || row.signed != model.row.basis_signed
                    || row.bit_offset != model.row.basis_bit_offset
                    || row.bit_width != model.row.basis_bit_width
                {
                    return err("holder basis does not match the active model metadata");
                }
            }
            let key = Key {
                contract: Some(row.market.clone()),
                address: row.holder.clone(),
            };
            if !row.previous_value.is_empty()
                && (!valid_decimal(&row.previous_value, row.signed)
                    || (!traversal.interrupted.contains(&row.market)
                        && self.state.models.get(&row.market).is_some_and(|model| model.effective_epoch == row.epoch)
                        && self
                            .state
                            .holders
                            .get(&key)
                            .is_some_and(|old| old.effective_epoch == row.epoch && old.row.value != row.previous_value)))
            {
                return err("holder observation is discontinuous with retained basis");
            }
            self.state.holders.insert(
                key,
                Fact {
                    row: row.clone(),
                    origin: origin.clone(),
                    observed_at: at.clone(),
                    effective_epoch: row.epoch,
                },
            );
            traversal.holders_done.insert(i);
        }
        let mut selected: BTreeMap<(GlobalKey, u32), &state::GlobalState> = BTreeMap::new();
        let mut changes: BTreeMap<(GlobalKey, u32), Vec<u64>> = BTreeMap::new();
        for (i, row) in events.global_state.iter().enumerate() {
            if traversal.globals_done.contains(&i) || !select(&row.market, row.epoch) {
                continue;
            }
            if row.market.len() != 20
                || !valid_decimal(&row.scale, false)
                || row.scale == "0"
                || row.bit_offset.checked_add(row.bit_width).is_none_or(|end| end > 256)
            {
                return err("global input requires a market, exact positive scale and valid bit range");
            }
            let key = (GlobalKey::of(row), row.epoch);
            let log = row.observation == state::Observation::ObservedLog as i32;
            if row.boundary == state::Boundary::Change as i32 && !log {
                changes.entry(key).or_default().push(row.ordinal);
                traversal.globals_done.insert(i);
                continue;
            }
            let constant = row.observation == state::Observation::QualifiedConstant as i32;
            if constant != (row.boundary == state::Boundary::Declaration as i32) || (constant && row.scope != state::Scope::Epoch as i32) {
                return err("qualified constants require explicit epoch declarations; other globals require end-of-block observations");
            }
            if constant
                && !events.epochs.iter().any(|e| {
                    e.market == row.market
                        && e.epoch == row.epoch
                        && matches!(
                            state::EpochEventKind::try_from(e.kind),
                            Ok(state::EpochEventKind::Bound | state::EpochEventKind::Reaffirmed)
                        )
                })
            {
                return err("qualified constant requires its model declaration in the same block");
            }
            if constant && self.state.models.get(&row.market).is_none_or(|model| model.effective_epoch != row.epoch) {
                // An unbound heartbeat is evidence, not qualification of a
                // constant. It cannot initialize evaluable model inputs.
                traversal.globals_done.insert(i);
                continue;
            }
            if let Some(previous) = selected.get(&key) {
                if !log || previous.ordinal == row.ordinal {
                    return err("duplicate or ambiguous final global input in one epoch and block");
                }
                if previous.ordinal > row.ordinal {
                    traversal.globals_done.insert(i);
                    continue;
                }
            }
            selected.insert(key, row);
            traversal.globals_done.insert(i);
        }
        for (key, ordinals) in changes {
            let Some(final_row) = selected.get(&key) else {
                return err("intermediate global changes require their final end-of-block row");
            };
            if !matches!(origin, Origin::Checkpoint { .. })
                && ordinals
                    .iter()
                    .any(|ordinal| *ordinal < final_row.first_ordinal || *ordinal > final_row.ordinal)
            {
                return err("final global range does not contain its intermediate effects");
            }
        }
        for ((key, _), row) in selected {
            if let Some(old) = self.state.globals.get(&key) {
                if old.effective_epoch == row.epoch && (old.row.scale != row.scale || old.row.signed != row.signed) {
                    return err("global input scale or sign changed within its epoch");
                }
                if key.observation == state::Observation::QualifiedConstant as i32 && old.row.value != row.value {
                    return err("qualified constant changed within its epoch");
                }
                if key.observation == state::Observation::ObservedWrite as i32
                    && old.effective_epoch == row.epoch
                    && !traversal.interrupted.contains(&row.market)
                    && self.state.models.get(&row.market).is_some_and(|model| model.effective_epoch == row.epoch)
                    && !row.previous_value.is_empty()
                    && row.previous_value != old.row.value
                {
                    return err("global observation is discontinuous with retained input");
                }
            }
            if !row.previous_value.is_empty() && !valid_decimal(&row.previous_value, row.signed) {
                return err("invalid previous global input");
            }
            if row.value.is_empty() {
                self.state.globals.remove(&key);
                continue;
            }
            self.state.globals.insert(
                key,
                Fact {
                    row: row.clone(),
                    origin: origin.clone(),
                    observed_at: at.clone(),
                    effective_epoch: row.epoch,
                },
            );
        }
        Ok(())
    }

    pub fn undo(&mut self, to_number: u64) -> Result<u64> {
        let mut staged = self.clone();
        let last = staged.ledger.last().ok_or_else(|| Error("nothing applied".into()))?.number;
        let needed = last.checked_sub(to_number).ok_or_else(|| Error("undo target is after current block".into()))?;
        if needed == 0 || needed > staged.history.len() as u64 {
            return err("protocol undo journal does not cover the target");
        }
        let undone = staged.ledger.undo(to_number)?;
        for _ in 0..undone {
            staged.state = staged.history.pop_back().expect("checked protocol journal");
        }
        *self = staged;
        Ok(undone)
    }

    fn available(&self, market: &[u8]) -> std::result::Result<u32, Unavailable> {
        if let Some(reason) = self.ledger.unsupported.get(market) {
            return Err(Unavailable::Unsupported(reason.clone()));
        }
        if let Some(s) = self.ledger.suspended.get(market) {
            return Err(Unavailable::Suspended {
                epoch: s.epoch,
                reason: s.reason,
            });
        }
        let model = self.state.models.get(market).ok_or(Unavailable::Unbound)?;
        if self.ledger.epoch(market) != Some(model.effective_epoch) {
            return Err(Unavailable::Unbound);
        }
        Ok(model.effective_epoch)
    }
    pub fn model(&self, market: &[u8]) -> std::result::Result<&Fact<state::ModelEpoch>, Unavailable> {
        self.available(market)?;
        self.state.models.get(market).ok_or(Unavailable::Unbound)
    }
    pub fn holder(&self, market: &[u8], holder: &[u8]) -> std::result::Result<&Fact<state::HolderBasis>, Unavailable> {
        let epoch = self.available(market)?;
        self.state
            .holders
            .get(&Key {
                contract: Some(market.to_vec()),
                address: holder.to_vec(),
            })
            .filter(|f| f.effective_epoch == epoch)
            .ok_or(Unavailable::Missing)
    }
    pub fn global(&self, key: &GlobalKey) -> std::result::Result<&Fact<state::GlobalState>, Unavailable> {
        let epoch = self.available(&key.market)?;
        self.state.globals.get(key).filter(|f| f.effective_epoch == epoch).ok_or(Unavailable::Missing)
    }
    pub fn dependencies(&self, market: &[u8]) -> std::result::Result<&[state::Dependency], Unavailable> {
        self.available(market)?;
        Ok(self.state.dependencies.get(market).map(Vec::as_slice).unwrap_or_default())
    }
    pub fn clock(&self) -> Option<&state::BlockClock> {
        self.state.clock.as_ref()
    }
    pub fn stream(&self) -> Option<&Stream> {
        self.ledger.stream.as_ref()
    }
    pub fn mark_unsupported(&mut self, market: &[u8], reason: &str) {
        self.ledger.mark_unsupported(market, reason);
    }
    pub fn report(&self) -> ProtocolReport {
        let count = |observation: state::Observation| {
            self.state
                .globals
                .keys()
                .filter(|k| k.observation == observation as i32 && self.available(&k.market).is_ok())
                .count()
        };
        ProtocolReport {
            raw_basis_ledger: self.ledger.report(),
            active_model_basis_holders: self
                .state
                .holders
                .iter()
                .filter(|(key, fact)| key.contract.as_ref().is_some_and(|market| self.available(market) == Ok(fact.effective_epoch)))
                .count(),
            bound_models: self.state.models.keys().filter(|m| self.available(m).is_ok()).count(),
            stored_globals: count(state::Observation::ObservedWrite),
            qualified_constants: count(state::Observation::QualifiedConstant),
            log_observations: count(state::Observation::ObservedLog),
            current_block_derived: count(state::Observation::Derived),
            timestamp: self.clock().map(|c| c.timestamp),
            undo_snapshots: self.history.len(),
        }
    }
}

fn validate_model(row: &state::ModelEpoch, clock: &state::BlockClock, allow_earlier_activation: bool) -> Result<()> {
    if row.market.len() != 20
        || !named::<state::ModelFamily>(row.family)
        || !named::<state::BasisKind>(row.basis_kind)
        || !named::<state::Rounding>(row.balance_rounding)
        || row.model_id.is_empty()
        || row.source_pin.is_empty()
        || row.basis_bit_width == 0
        || row.basis_bit_offset.checked_add(row.basis_bit_width).is_none_or(|end| end > 256)
        || row.basis_signed != (row.basis_kind == state::BasisKind::SignedPrincipal as i32)
        || (!row.basis_scale.is_empty() && (!valid_decimal(&row.basis_scale, false) || row.basis_scale == "0"))
    {
        return err("incomplete or invalid protocol model binding");
    }
    for (value, width) in [
        (&row.implementation, 20),
        (&row.implementation_slot, 32),
        (&row.implementation_code_hash, 32),
        (&row.market_code_hash, 32),
        (&row.balance_asset, 20),
    ] {
        if !value.is_empty() && value.len() != width {
            return err("malformed optional model identity");
        }
    }
    if row.activation_block > clock.number || (!allow_earlier_activation && (row.activation_block != clock.number || row.activation_ordinal != row.ordinal)) {
        return err("model activation does not match its BOUND position");
    }
    Ok(())
}

fn dependency_set(mut rows: Vec<state::Dependency>) -> Vec<state::Dependency> {
    for row in &mut rows {
        row.kind = state::EpochEventKind::Bound as i32;
    }
    rows.sort_by(|a, b| (&a.market, a.epoch, a.role, &a.contract, &a.parent).cmp(&(&b.market, b.epoch, b.role, &b.contract, &b.parent)));
    rows
}
