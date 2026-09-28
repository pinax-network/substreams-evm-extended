//! Execution-position checks for host consumers. Checkpoints describe
//! independently initialized inputs, not effects of their snapshot block.
use super::{err, state, Clock, Result};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Input {
    Emitted,
    #[cfg(not(target_arch = "wasm32"))]
    Checkpoint,
}

struct Span {
    first: u64,
    last: u64,
    count: u32,
}

impl Span {
    fn validate(&self, clock: &Clock, events: &state::Events, market: &[u8], epoch: u32, input: Input) -> Result<()> {
        if self.first > self.last {
            return err("observation span has reversed execution ordinals");
        }
        if input != Input::Emitted {
            // The provenance may be historical or a neutral snapshot
            // descriptor. Never assign it a fictitious current-block write.
            return Ok(());
        }
        if self.first == 0 || self.count == 0 {
            return err("emitted observation requires positive ordinals and a nonzero change count");
        }
        // Exact epoch membership and lifecycle ordering were already checked.
        // Skipped IDs do not introduce boundaries: only actual BOUND rows do.
        let bound = events
            .epochs
            .iter()
            .filter(|e| e.market == market && e.epoch == epoch && e.kind == state::EpochEventKind::Bound as i32)
            .map(|e| e.ordinal)
            .max();
        let activation = events
            .epochs
            .iter()
            .filter(|e| e.market == market && e.epoch == epoch && e.activation_block == clock.number)
            .map(|e| e.activation_ordinal)
            .max()
            .unwrap_or(0);
        let start = bound.unwrap_or(0).max(activation);
        let successor = events
            .epochs
            .iter()
            .filter(|e| e.market == market && e.epoch > epoch && e.kind == state::EpochEventKind::Bound as i32)
            .map(|e| e.ordinal)
            .min();
        let cutoff = events
            .epochs
            .iter()
            .filter(|e| {
                e.market == market
                    && e.epoch == epoch
                    && matches!(state::EpochEventKind::try_from(e.kind), Ok(state::EpochEventKind::Invalidated | state::EpochEventKind::Suspended))
                    // An unbound, suspended identity may first bind this same
                    // ID later. Earlier suspension cannot end that new binding.
                    && bound.is_none_or(|start| e.ordinal >= start)
            })
            .map(|e| e.ordinal)
            .min();
        if self.first < start || successor.is_some_and(|end| self.last >= end) || cutoff.is_some_and(|end| self.last >= end) {
            return err(format!(
                "observation span {}..={} lies outside epoch {epoch}'s execution interval",
                self.first, self.last
            ));
        }
        // Do not consult a previous block's suspension ordinal: stateless
        // later output stays quarantined, and rebinding still discards it.
        Ok(())
    }

    fn single_effect(&self) -> Result<()> {
        if self.first != self.last || self.count != 1 {
            return err("CHANGE observation must identify one execution ordinal and one effect");
        }
        Ok(())
    }
}

pub(super) fn validate(clock: &Clock, events: &state::Events, input: Input) -> Result<()> {
    for row in &events.holder_basis {
        Span {
            first: row.first_ordinal,
            last: row.ordinal,
            count: row.change_count,
        }
        .validate(clock, events, &row.market, row.epoch, input)?;
    }
    for row in &events.global_state {
        let span = Span {
            first: row.first_ordinal,
            last: row.ordinal,
            count: row.change_count,
        };
        if row.observation == state::Observation::QualifiedConstant as i32 {
            if input == Input::Emitted
                && (row.boundary != state::Boundary::Declaration as i32
                    || row.scope != state::Scope::Epoch as i32
                    || row.change_count != 0
                    || (row.first_ordinal != 0 && row.first_ordinal != row.ordinal)
                    || !events.epochs.iter().any(|e| {
                        e.market == row.market
                            && e.epoch == row.epoch
                            && e.ordinal == row.ordinal
                            && matches!(
                                state::EpochEventKind::try_from(e.kind),
                                Ok(state::EpochEventKind::Bound | state::EpochEventKind::Reaffirmed)
                            )
                    }))
            {
                return err("emitted constant must match its declaration position and claim no writes");
            }
            continue;
        }
        span.validate(clock, events, &row.market, row.epoch, input)?;
        if input != Input::Emitted {
            continue;
        }
        match (state::Observation::try_from(row.observation), state::Boundary::try_from(row.boundary)) {
            (Ok(state::Observation::ObservedWrite | state::Observation::Derived), Ok(state::Boundary::EndOfBlock)) => {}
            (Ok(state::Observation::ObservedWrite | state::Observation::ObservedLog), Ok(state::Boundary::Change)) => span.single_effect()?,
            _ => return err("global observation has an incompatible execution boundary"),
        }
    }
    Ok(())
}
