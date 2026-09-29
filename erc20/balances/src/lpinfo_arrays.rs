//! Selected TOPS runtime only. Complete bounded append/cleanup witnesses grant
//! individual metadata events, never an array range or an independent credit.
//! Equality omissions supply constraints, not guessed historical storage.
use crate::{eth, hash, require, word, VerifiedLayout};
use std::collections::{BTreeMap, BTreeSet};
use substreams::errors::Error;

type Word = [u8; 32];
type Accepted = BTreeSet<(Vec<u8>, Word, u64)>;
const ZERO: Word = [0; 32];
const LIMIT: usize = 6;
fn n(value: u64) -> Word {
    let mut v = ZERO;
    v[24..].copy_from_slice(&value.to_be_bytes());
    v
}
fn plus(a: Word, b: Word) -> Word {
    let mut out = ZERO;
    let mut carry = 0u16;
    for i in (0..32).rev() {
        carry += u16::from(a[i]) + u16::from(b[i]);
        out[i] = carry as u8;
        carry >>= 8;
    }
    out
}
fn mapping(owner: Word, root: Word) -> Word {
    hash(&[owner.as_slice(), root.as_slice()].concat())
}
#[derive(Clone)]
struct Owner {
    address: Word,
    head: Word,
    base: Word,
    credit: Word,
}
impl Owner {
    fn new(address: Word) -> Self {
        let head = mapping(address, n(31));
        Self {
            address,
            head,
            base: hash(&head),
            credit: mapping(address, n(32)),
        }
    }
    fn element(&self, index: usize, field: usize) -> Word {
        plus(self.base, n((3 * index + field) as u64))
    }
    fn keys(&self) -> Vec<Word> {
        let mut out = vec![self.head, self.credit];
        out.extend((0..LIMIT).flat_map(|i| (0..3).map(move |f| self.element(i, f))));
        out
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Slot {
    key: Word,
    old: Word,
    new: Word,
}
struct Event {
    frame: usize,
    account: Vec<u8>,
    slot: Slot,
    ordinal: u64,
}
struct Frame {
    group: usize,
    begin: u64,
    end: u64,
    address: Vec<u8>,
    input: Vec<u8>,
    origin: Option<Word>,
}
fn collect(block: &eth::Block, selected: &BTreeSet<Vec<u8>>) -> Result<(Vec<Frame>, Vec<Event>), Error> {
    let mut frames = vec![];
    let mut events = vec![];
    let mut ordinals = BTreeSet::new();
    let mut add = |call: &eth::Call, group, persists, origin| -> Result<(), Error> {
        let frame = frames.len();
        frames.push(Frame {
            group,
            begin: call.begin_ordinal,
            end: call.end_ordinal,
            address: call.address.clone(),
            input: call.input.clone(),
            origin,
        });
        if persists && !call.state_reverted {
            for row in &call.storage_changes {
                let slot = if selected.contains(&row.address) {
                    require(row.key.len() == 32, "LPInfo storage key must be exactly32 bytes")?;
                    require(
                        row.ordinal > 0 && ordinals.insert(row.ordinal),
                        "LPInfo selected ordinal is missing or duplicated",
                    )?;
                    require(
                        call.begin_ordinal > 0 && call.begin_ordinal < row.ordinal && row.ordinal < call.end_ordinal,
                        "LPInfo selected write outside frame",
                    )?;
                    Slot {
                        key: word(&row.key)?,
                        old: word(&row.old_value)?,
                        new: word(&row.new_value)?,
                    }
                } else {
                    Slot {
                        key: ZERO,
                        old: ZERO,
                        new: ZERO,
                    }
                };
                events.push(Event {
                    frame,
                    account: row.address.clone(),
                    slot,
                    ordinal: row.ordinal,
                });
            }
        }
        Ok(())
    };
    for (group, tx) in block.transaction_traces.iter().enumerate() {
        let origin = if tx.from.len() == 20 { Some(word(&tx.from)?) } else { None };
        for call in &tx.calls {
            add(call, group, tx.status() == eth::TransactionTraceStatus::Succeeded, origin)?;
        }
    }
    for call in &block.system_calls {
        add(call, block.transaction_traces.len(), true, None)?;
    }
    events.sort_by_key(|e| e.ordinal);
    Ok((frames, events))
}

// At most 18 original array words. Only observed words and prior accepted
// block-local facts ground values. An omission is a deferred equality check:
// it must never invent old zero from a missing clear or copy an unknown value.
#[derive(Clone)]
struct Solve {
    values: [Option<Word>; LIMIT * 3],
    omitted: Vec<(Expr, Expr)>,
}
#[derive(Clone, Copy)]
enum Expr {
    Value(Word),
    Original(usize),
}
impl Solve {
    fn new() -> Self {
        Self {
            values: [None; LIMIT * 3],
            omitted: vec![],
        }
    }
    fn value(&self, e: Expr) -> Option<Word> {
        match e {
            Expr::Value(v) => Some(v),
            Expr::Original(i) => self.values[i],
        }
    }
    fn equal(&mut self, a: Expr, b: Expr) -> bool {
        if let (Some(a), Some(b)) = (self.value(a), self.value(b)) {
            return a == b;
        }
        match (a, b) {
            (Expr::Original(i), Expr::Value(v)) | (Expr::Value(v), Expr::Original(i)) => {
                self.values[i] = Some(v);
            }
            _ => unreachable!(),
        }
        true
    }
}
#[derive(Clone)]
struct Stage {
    key: Word,
    old: Expr,
    new: Expr,
}
fn constant(key: Word, old: Word, new: Word) -> Stage {
    Stage {
        key,
        old: Expr::Value(old),
        new: Expr::Value(new),
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Operation {
    end: usize,
    logical: Vec<Slot>,
}
#[allow(clippy::too_many_arguments)]
fn match_stages(stages: &[Stage], position: usize, events: &[Event], account: &[u8], frame: usize, solve: Solve, output: &mut Vec<(usize, Solve)>) {
    let Some((stage, rest)) = stages.split_first() else {
        output.push((position, solve));
        return;
    };
    let mut omitted = solve.clone();
    if !matches!((omitted.value(stage.old), omitted.value(stage.new)), (Some(a),Some(b)) if a != b) {
        omitted.omitted.push((stage.old, stage.new));
        match_stages(rest, position, events, account, frame, omitted, output);
    }
    if let Some(e) = events
        .get(position)
        .filter(|e| e.account == account && e.frame == frame && e.slot.key == stage.key)
    {
        let mut consumed = solve;
        if consumed.equal(stage.old, Expr::Value(e.slot.old)) && consumed.equal(stage.new, Expr::Value(e.slot.new)) {
            match_stages(rest, position + 1, events, account, frame, consumed, output);
        }
    }
}
fn materialize(stages: &[Stage], solve: &Solve) -> Option<Vec<Slot>> {
    if !solve
        .omitted
        .iter()
        .all(|(a, b)| matches!((solve.value(*a),solve.value(*b)),(Some(a),Some(b)) if a == b))
    {
        return None;
    }
    stages
        .iter()
        .map(|s| {
            Some(Slot {
                key: s.key,
                old: solve.value(s.old)?,
                new: solve.value(s.new)?,
            })
        })
        .collect()
}
fn selector(input: &[u8], signature: &[u8], minimum: usize) -> bool {
    input.len() >= minimum && input[..4] == hash(signature)[..4]
}
fn candidates(start: usize, events: &[Event], frames: &[Frame], owner: &Owner, now: Word, known: &BTreeMap<Word, Word>) -> Vec<Operation> {
    let first = &events[start];
    let frame = &frames[first.frame];
    let mut out = vec![];
    if frame.address != first.account {
        return out;
    }
    // An external source append has a non-equal head and amount. Matching its
    // effects does not independently attest deployed miner/caller authorization.
    if selector(&frame.input, b"createLPInfo(address,uint256)", 68)
        && frame.input[4..36] == owner.address
        && owner.address != ZERO
        && first.slot.key == owner.head
    {
        let amount: Word = frame.input[36..68].try_into().unwrap();
        let expiry = plus(now, n(8_640_000));
        if amount != ZERO && expiry >= now {
            for length in 0..LIMIT {
                let stages = vec![
                    constant(owner.head, n(length as u64), n(length as u64 + 1)),
                    Stage {
                        key: owner.element(length, 0),
                        old: Expr::Original(3 * length),
                        new: Expr::Value(amount),
                    },
                    Stage {
                        key: owner.element(length, 1),
                        old: Expr::Original(3 * length + 1),
                        new: Expr::Value(now),
                    },
                    Stage {
                        key: owner.element(length, 2),
                        old: Expr::Original(3 * length + 2),
                        new: Expr::Value(expiry),
                    },
                ];
                let mut matched = vec![];
                let mut seeded = Solve::new();
                for f in 0..3 {
                    seeded.values[3 * length + f] = known.get(&owner.element(length, f)).copied();
                }
                match_stages(&stages, start, events, &first.account, first.frame, seeded, &mut matched);
                for (end, solve) in matched {
                    if end > start {
                        if let Some(logical) = materialize(&stages, &solve) {
                            if logical[1..].iter().all(|s| s.old == ZERO) {
                                out.push(Operation { end, logical });
                            }
                        }
                    }
                }
            }
        }
    }
    if frame.origin != Some(owner.address)
        || !((selector(&frame.input, b"transfer(address,uint256)", 68) && frame.input[4..16] == [0; 12])
            || (selector(&frame.input, b"transferFrom(address,address,uint256)", 100) && frame.input[4..16] == [0; 12] && frame.input[36..48] == [0; 12]))
    {
        return out;
    }
    // No admitted cleanup has more than 25 stores. Its credit must change, so
    // an omitted/standalone credit cannot serve as an operation witness.
    for credit in events
        .iter()
        .skip(start)
        .take(25)
        .filter(|e| e.frame == first.frame && e.account == first.account && e.slot.key == owner.credit && e.slot.old != e.slot.new)
    {
        for length in 1..=LIMIT {
            for removed in 1..=length {
                let remaining = length - removed;
                let mut stages = vec![];
                for i in 0..remaining {
                    for f in 0..3 {
                        stages.push(Stage {
                            key: owner.element(i, f),
                            old: Expr::Original(3 * i + f),
                            new: Expr::Original(3 * (i + removed) + f),
                        });
                    }
                }
                for i in (remaining..length).rev() {
                    for f in 0..3 {
                        stages.push(Stage {
                            key: owner.element(i, f),
                            old: Expr::Original(3 * i + f),
                            new: Expr::Value(ZERO),
                        });
                    }
                    stages.push(constant(owner.head, n(i as u64 + 1), n(i as u64)));
                }
                stages.push(constant(owner.credit, credit.slot.old, credit.slot.new));
                let mut matched = vec![];
                let mut seeded = Solve::new();
                for i in 0..length {
                    for f in 0..3 {
                        seeded.values[3 * i + f] = known.get(&owner.element(i, f)).copied();
                    }
                }
                match_stages(&stages, start, events, &first.account, first.frame, seeded, &mut matched);
                for (end, solve) in matched {
                    let Some(original) = (0..3 * length).map(|i| solve.value(Expr::Original(i))).collect::<Option<Vec<_>>>() else {
                        continue;
                    };
                    let prefix = original.chunks_exact(3).take_while(|r| r[2] <= now).count();
                    let sum = original.chunks_exact(3).take(removed).fold(ZERO, |sum, r| plus(sum, r[0]));
                    if prefix != removed || sum == ZERO || plus(credit.slot.old, sum) != credit.slot.new {
                        continue;
                    }
                    if let Some(logical) = materialize(&stages, &solve) {
                        out.push(Operation { end, logical });
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn protected(layout: &VerifiedLayout, preimages: &BTreeMap<Word, Vec<u8>>, balances: &BTreeMap<Word, BTreeMap<Word, Vec<u8>>>) -> BTreeSet<Word> {
    let mut roots = BTreeSet::from([layout.balance_slot, n(31), n(32)]);
    roots.extend(&layout.other_slots);
    roots.extend(&layout.other_mapping_slots);
    roots.extend(layout.other_mapping_words.keys());
    roots.extend(layout.other_mapping_paths.iter().map(|p| p.root));
    roots.extend(&layout.address_lists);
    let mut set = roots.clone();
    if let Some(leaves) = balances.get(&layout.balance_slot) {
        set.extend(leaves.keys());
    }
    // All verified descendants of preserved fields are protected, including
    // nested allowances; no equality omission may consume an alias there.
    loop {
        let mut changed = false;
        for (key, preimage) in preimages {
            if preimage.len() == 64 {
                let parent: Word = preimage[32..].try_into().unwrap();
                if set.contains(&parent) && parent != n(31) && parent != n(32) {
                    changed |= set.insert(*key);
                }
            }
        }
        if !changed {
            break;
        }
    }
    set
}
fn scope(start: usize, end: usize, events: &[Event], frames: &[Frame]) -> Result<(), Error> {
    let first = &events[start];
    let last = &events[end - 1];
    let frame = &frames[first.frame];
    require(
        frame.begin > 0 && frame.end > frame.begin && first.ordinal > frame.begin && last.ordinal < frame.end,
        "LPInfo operation outside frame",
    )?;
    let mut ordinals = BTreeSet::new();
    let mut group = vec![];
    for (id, other) in frames.iter().enumerate() {
        if other.group == frame.group {
            require(other.begin > 0 && other.end > other.begin, "LPInfo group has ambiguous frame")?;
            group.push(id);
        }
        if id != first.frame {
            require(
                ![other.begin, other.end].into_iter().any(|o| first.ordinal <= o && o <= last.ordinal && o > 0),
                "LPInfo crosses a call boundary",
            )?;
        }
    }
    for (i, &a) in group.iter().enumerate() {
        for &b in &group[i + 1..] {
            let (a, b) = (&frames[a], &frames[b]);
            require(
                a.end <= b.begin || b.end <= a.begin || (a.begin < b.begin && b.end < a.end) || (b.begin < a.begin && a.end < b.end),
                "LPInfo group frames overlap ambiguously",
            )?;
        }
    }
    for e in events.iter().filter(|e| frames[e.frame].group == frame.group) {
        let f = &frames[e.frame];
        require(
            e.ordinal > f.begin && e.ordinal < f.end && ordinals.insert(e.ordinal),
            "LPInfo storage barrier ordinal/frame is ambiguous",
        )?;
    }
    require(
        events[start..end].iter().all(|e| e.frame == first.frame && e.account == first.account),
        "LPInfo crosses a foreign store",
    )?;
    require(
        events.iter().filter(|e| first.ordinal <= e.ordinal && e.ordinal <= last.ordinal).count() == end - start,
        "LPInfo has an inclusive foreign or tied storage barrier",
    )?;
    Ok(())
}

pub fn validate(
    block: &eth::Block,
    layouts: &[VerifiedLayout],
    preimages: &BTreeMap<Word, Vec<u8>>,
    balances: &BTreeMap<Word, BTreeMap<Word, Vec<u8>>>,
    address_lists: &BTreeSet<(Vec<u8>, Word)>,
) -> Result<Accepted, Error> {
    let selected: Vec<_> = layouts.iter().filter(|l| l.lpinfo_array).collect();
    if selected.is_empty() {
        return Ok(BTreeSet::new());
    }
    require(matches!(block.ver, 4 | 5), "LPInfo requires Extended producer v4/v5")?;
    let timestamp = block
        .header
        .as_ref()
        .and_then(|h| h.timestamp.as_ref())
        .ok_or_else(|| Error::msg("LPInfo timestamp missing"))?;
    require(timestamp.seconds >= 0 && timestamp.nanos == 0, "LPInfo timestamp is not canonical seconds")?;
    let now = n(timestamp.seconds as u64);
    let (frames, events) = collect(block, &selected.iter().map(|l| l.contract.clone()).collect())?;
    let mut accepted = BTreeSet::new();
    for layout in selected {
        let mut owners = BTreeMap::new();
        let mut witnessed_heads = BTreeSet::new();
        let mut forbidden = BTreeSet::from([n(31), n(32)]);
        for (key, p) in preimages {
            require(hash(p) == *key, "LPInfo preimage hash mismatch")?;
            if p.len() == 64 && (p[32..] == n(31) || p[32..] == n(32)) {
                if p[..12] != [0; 12] {
                    forbidden.insert(*key);
                    continue;
                }
                let owner: Word = p[..32].try_into().unwrap();
                owners.entry(owner).or_insert_with(|| Owner::new(owner));
                if p[32..] == n(31) {
                    witnessed_heads.insert(owner);
                }
            }
        }
        let mut protected = protected(layout, preimages, balances);
        protected.extend(address_lists.iter().filter(|(account, _)| account == &layout.contract).map(|(_, key)| *key));
        let mut keys = BTreeMap::new();
        for (id, owner) in &owners {
            for key in owner.keys() {
                require(
                    !protected.contains(&key) && keys.insert(key, *id).is_none(),
                    "LPInfo namespace aliases another field or owner",
                )?;
            }
        }
        // Reject deeper mapping shapes even where a broad fallback could match.
        loop {
            let mut changed = false;
            for (key, p) in preimages {
                if p.len() == 64 {
                    let parent: Word = p[32..].try_into().unwrap();
                    if (forbidden.contains(&parent) || keys.contains_key(&parent)) && !keys.contains_key(key) {
                        changed |= forbidden.insert(*key);
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let mut known = BTreeMap::new();
        let mut operation_frames = BTreeSet::new();
        let mut physical = BTreeMap::new();
        for event in events.iter().filter(|e| e.account == layout.contract) {
            require(
                physical.get(&event.slot.key).is_none_or(|v| *v == event.slot.old),
                "LPInfo selected physical storage is discontinuous",
            )?;
            physical.insert(event.slot.key, event.slot.new);
        }
        let mut position = 0;
        while position < events.len() {
            let event = &events[position];
            if event.account != layout.contract {
                position += 1;
                continue;
            }
            require(!forbidden.contains(&event.slot.key), "unsupported LPInfo namespace write")?;
            let Some(owner) = keys.get(&event.slot.key).map(|id| &owners[id]) else {
                position += 1;
                continue;
            };
            require(witnessed_heads.contains(&owner.address), "LPInfo operation lacks exact root31 head preimage")?;
            let found: Vec<_> = candidates(position, &events, &frames, owner, now, &known)
                .into_iter()
                .filter(|op| {
                    let mut staged = known.clone();
                    op.logical.iter().all(|s| {
                        if staged.get(&s.key).is_some_and(|v| *v != s.old) {
                            return false;
                        }
                        staged.insert(s.key, s.new);
                        true
                    })
                })
                .collect();
            require(found.len() == 1, "LPInfo operation missing, inconsistent or ambiguous")?;
            let op = &found[0];
            scope(position, op.end, &events, &frames)?;
            // Each selected source entry point invokes exactly one append or
            // cleanup. Separate successful calls may still share this block.
            require(operation_frames.insert(event.frame), "LPInfo frame contains more than one operation")?;
            for s in &op.logical {
                require(
                    !protected.contains(&s.key) && keys.get(&s.key) == Some(&owner.address),
                    "LPInfo logical stage aliases another field",
                )?;
                known.insert(s.key, s.new);
            }
            for e in &events[position..op.end] {
                accepted.insert((e.account.clone(), e.slot.key, e.ordinal));
            }
            position = op.end;
        }
    }
    Ok(accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protects_every_logical_array_word_against_balance_and_list_aliases() {
        // Inject the independently classified physical-key sets at the exact
        // validator boundary. This tests collision handling without claiming
        // to construct an actual Keccak collision or inventing preimages.
        let mut profile = serde_json::from_str::<Vec<serde_json::Value>>(include_str!("../tests/fixtures/bsc-refined450-layouts.json"))
            .unwrap()
            .into_iter()
            .find(|v| v["contract"] == "0xcdf52c0b13c24f32f1d8d4ec6356203a1ef0826a")
            .unwrap();
        profile["other_mapping_slots"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != &format!("0x{}", hex::encode(n(32))));
        profile["lpinfo_array"] = serde_json::json!(crate::layout::TOPS_LPINFO_SEMANTICS);
        let layouts = crate::layout::parse(&serde_json::json!([profile]).to_string()).unwrap();
        let layout = &layouts[0];
        let owner = Owner::new(n(55));
        let preimages = BTreeMap::from([(owner.head, [owner.address.as_slice(), n(31).as_slice()].concat())]);
        let block = eth::Block {
            ver: 5,
            header: Some(eth::BlockHeader {
                timestamp: Some(prost_types::Timestamp { seconds: 100, nanos: 0 }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(validate(&block, &layouts, &preimages, &BTreeMap::new(), &BTreeSet::new()).is_ok());
        for key in owner.keys() {
            let balances = BTreeMap::from([(n(5), BTreeMap::from([(key, vec![7; 20])]))]);
            assert!(validate(&block, &layouts, &preimages, &balances, &BTreeSet::new()).is_err());
            let lists = BTreeSet::from([(layout.contract.clone(), key)]);
            assert!(validate(&block, &layouts, &preimages, &BTreeMap::new(), &lists).is_err());
            let unrelated = BTreeSet::from([(vec![9; 20], key)]);
            assert!(validate(&block, &layouts, &preimages, &BTreeMap::new(), &unrelated).is_ok());
        }
        // Canonical nested allowance leaves are also included in protection,
        // even when they have no physical write in this block.
        let outer = mapping(n(44), n(6));
        let inner = mapping(n(22), outer);
        let mut preimages = preimages;
        preimages.insert(outer, [n(44).as_slice(), n(6).as_slice()].concat());
        preimages.insert(inner, [n(22).as_slice(), outer.as_slice()].concat());
        let protected = protected(layout, &preimages, &BTreeMap::new());
        assert!(protected.contains(&outer) && protected.contains(&inner));
    }
}
