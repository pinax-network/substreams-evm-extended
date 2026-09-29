//! Exact observed-write witnesses for legacy DSG/OZ3.4.2 and the separately
//! opted-in PTokenV2 solc0.8.28/OZ5.4, SecuritiesToken solc0.8.24/OZ5.3 and
//! GM solc0.8.16/Ondo-vendor coupled membership/set templates. Names identify
//! selected reviewed builds, not every contract using a library version.
//!
//! The caller binds the runtime and an outer mapping(bytes32 => RoleData) root.
//! This rule grants event permissions, never an array range or an inferred key.
//! Its block-local constraints do not reconstruct untouched historical members.
//! The qualified runtime must establish the empty-unused-tail invariant; a zero
//! member remains valid. Unknown no-op keys retain the mapper's ordinary policy.
//! Checked logical growth is a conservative admission restriction: the selected
//! compilers actually wrap a maximum-length push outside coherent state.
use crate::{eth, hash, require, word, VerifiedLayout};
use std::collections::{BTreeMap, BTreeSet};
use substreams::errors::Error;

type Word = [u8; 32];
type Key = (Vec<u8>, Word);
type Accepted = BTreeSet<(Vec<u8>, Word, u64)>;
const ZERO: Word = [0; 32];
const ONE: Word = one();

const fn one() -> Word {
    let mut value = [0; 32];
    value[31] = 1;
    value
}

// Physical storage positions wrap modulo 2^256. Logical lengths do not.
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

fn increment(a: Word) -> Option<Word> {
    (a != [255; 32]).then(|| plus(a, ONE))
}

fn decrement(mut a: Word) -> Option<Word> {
    if a == ZERO {
        return None;
    }
    for byte in a.iter_mut().rev() {
        let (next, borrow) = byte.overflowing_sub(1);
        *byte = next;
        if !borrow {
            break;
        }
    }
    Some(a)
}

fn mapping(member: Word, root: Word) -> Word {
    let mut input = [0; 64];
    input[..32].copy_from_slice(&member);
    input[32..].copy_from_slice(&root);
    hash(&input)
}

#[derive(Clone, Copy)]
struct Frame {
    group: usize,
    begin: u64,
    end: u64,
}

struct Event {
    frame: usize,
    account: Vec<u8>,
    slot: Slot,
    ordinal: u64,
}

struct Collected {
    frames: Vec<Frame>,
    events: Vec<Event>,
    // Keep all persisted accounts as ordering barriers without copying their
    // storage payloads. A source operation cannot contain a foreign SSTORE.
    storage_ordinals: Vec<u64>,
    storage_frames: Vec<(usize, u64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    key: Word,
    old: Word,
    new: Word,
}

struct Role {
    account: Vec<u8>,
    array: Word,
    index: Word,
    membership: Option<Word>,
    complete_roots: bool,
}

#[derive(Clone, Copy)]
enum Namespace {
    Head(usize),
    Index(usize, Word),
    Membership(usize, Word),
    Forbidden,
}

struct Namespaces {
    roles: Vec<Role>,
    names: BTreeMap<Key, Namespace>,
    protected: BTreeSet<Key>,
}

#[derive(Clone, PartialEq, Eq)]
struct Operation {
    role: usize,
    head: usize,
    observed: Vec<usize>,
    logical: Vec<Stage>,
    // Source-implied coherence only. These are not SSTORE stages or permissions.
    constraints: Vec<Slot>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StageKind {
    Length,
    Element,
    Position,
    Membership,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stage {
    slot: Slot,
    kind: StageKind,
}
fn stages<const N: usize>(slots: [Slot; N], kinds: [StageKind; N]) -> Vec<Stage> {
    slots.into_iter().zip(kinds).map(|(slot, kind)| Stage { slot, kind }).collect()
}

fn collect(block: &eth::Block, accounts: &BTreeSet<Vec<u8>>, coupled_accounts: &BTreeSet<Vec<u8>>) -> Result<Collected, Error> {
    let mut frames = Vec::new();
    let mut events = Vec::new();
    let mut ordinals = BTreeSet::new();
    let mut storage_ordinals = Vec::new();
    let mut storage_frames = Vec::new();
    let mut add = |call: &eth::Call, group: usize, persists: bool, begin: u64| -> Result<(), Error> {
        let frame = frames.len();
        frames.push(Frame {
            group,
            begin,
            end: call.end_ordinal,
        });
        if persists && !call.state_reverted {
            for row in &call.storage_changes {
                storage_ordinals.push(row.ordinal);
                storage_frames.push((frame, row.ordinal));
                if !accounts.contains(&row.address) {
                    continue;
                }
                require(
                    !coupled_accounts.contains(&row.address) || row.key.len() == 32,
                    "coupled enumerable storage key must be exactly32 bytes",
                )?;
                require(
                    row.ordinal > 0 && ordinals.insert(row.ordinal),
                    "enumerable-set storage ordinal is missing or duplicated",
                )?;
                require(
                    begin > 0 && begin < row.ordinal && row.ordinal < call.end_ordinal,
                    "enumerable-set storage is outside its execution frame",
                )?;
                events.push(Event {
                    frame,
                    account: row.address.clone(),
                    slot: Slot {
                        key: word(&row.key)?,
                        old: word(&row.old_value)?,
                        new: word(&row.new_value)?,
                    },
                    ordinal: row.ordinal,
                });
            }
        }
        Ok(())
    };
    for (position, tx) in block.transaction_traces.iter().enumerate() {
        let persists = tx.status() == eth::TransactionTraceStatus::Succeeded;
        for (call_position, call) in tx.calls.iter().enumerate() {
            // The Extended v3 protobuf documents a zero root-call begin and
            // prescribes the transaction begin as its replacement. This does
            // not normalize other calls, system calls, v4/v5 or unknown zeros.
            let begin = if block.ver == 3 && call_position == 0 && call.begin_ordinal == 0 {
                tx.begin_ordinal
            } else {
                call.begin_ordinal
            };
            add(call, position, persists, begin)?;
        }
    }
    // One distinct execution group for system calls; each has its own structural
    // frame ID. Declared transaction/call indices are never identity witnesses.
    for call in &block.system_calls {
        add(call, block.transaction_traces.len(), true, call.begin_ordinal)?;
    }
    storage_ordinals.sort_unstable();
    Ok(Collected {
        frames,
        events,
        storage_ordinals,
        storage_frames,
    })
}

// These are token-account roots, matching the parser's reservation scope.
// Beacon implementation/admin pointers live in the beacon account, not here.
fn protected_roots(layout: &VerifiedLayout) -> BTreeSet<Word> {
    let mut roots = BTreeSet::from([layout.balance_slot]);
    roots.extend(&layout.other_slots);
    roots.extend(&layout.other_mapping_slots);
    roots.extend(layout.other_mapping_words.keys());
    roots.extend(layout.other_mapping_paths.iter().map(|path| path.root));
    roots.extend(&layout.address_lists);
    roots.extend(layout.enumerable_address_sets.iter().map(|set| set.root));
    roots.extend(layout.enumerable_address_sets.iter().filter_map(|set| set.semantics.membership_root()));
    roots.extend(layout.proxy.as_ref().map(|proxy| proxy.implementation_slot));
    roots.extend(layout.beacon_proxy.as_ref().map(|proxy| proxy.beacon_slot));
    roots.extend(layout.zero_balance.as_ref().and_then(|rule| rule.storage_slot));
    roots.extend(layout.balance_divisor.as_ref().map(|rule| rule.storage_slot));
    if let Some(rule) = &layout.address_hash_balance {
        roots.extend(rule.stored_addresses.keys());
    }
    if let Some(rule) = &layout.voting_checkpoints {
        roots.extend(&rule.slots);
        roots.extend(&rule.mapping_slots);
    }
    roots
}

fn namespaces(
    layouts: &[VerifiedLayout],
    preimages: &BTreeMap<Word, Vec<u8>>,
    balance_candidates: &BTreeMap<Word, BTreeMap<Word, Vec<u8>>>,
) -> Result<Namespaces, Error> {
    let mut roles = Vec::new();
    let mut names = BTreeMap::new();
    let mut protected = BTreeSet::new();
    let mut by_parent = BTreeMap::<Word, Vec<(Word, Word)>>::new();
    for (key, bytes) in preimages {
        require(hash(bytes) == *key, "enumerable-set preimage hash mismatch")?;
        if bytes.len() == 64 {
            by_parent
                .entry(bytes[32..].try_into().unwrap())
                .or_default()
                .push((*key, bytes[..32].try_into().unwrap()));
        }
    }
    for layout in layouts.iter().filter(|layout| !layout.enumerable_address_sets.is_empty()) {
        protected.extend(protected_roots(layout).into_iter().map(|root| (layout.contract.clone(), root)));
        // Holder hints identify balance leaves even when their hash preimages
        // are absent. Protect logical operation keys here, before unchanged
        // writes can be filtered from ordinary balance processing.
        if let Some(leaves) = balance_candidates.get(&layout.balance_slot) {
            protected.extend(leaves.keys().map(|leaf| (layout.contract.clone(), *leaf)));
        }
        for (leaf, member) in by_parent.get(&layout.balance_slot).into_iter().flatten() {
            if member[..12] == [0; 12] {
                protected.insert((layout.contract.clone(), *leaf));
            }
        }
        for set in &layout.enumerable_address_sets {
            let membership_root = set.semantics.membership_root();
            // Either side discovers the role. Derived counterparts identify
            // fragments but never replace a missing permission preimage.
            let mut outer = BTreeSet::new();
            outer.extend(by_parent.get(&set.root).into_iter().flatten().map(|(_, r)| *r));
            if let Some(root) = membership_root {
                outer.extend(by_parent.get(&root).into_iter().flatten().map(|(_, r)| *r));
            }
            for role_key in outer {
                let head = mapping(role_key, set.root);
                let membership = membership_root.map(|root| mapping(role_key, root));
                let complete_roots = preimages.contains_key(&head) && membership.is_none_or(|base| preimages.contains_key(&base));
                let id = roles.len();
                let index = plus(head, ONE);
                let role = Role {
                    account: layout.contract.clone(),
                    array: hash(&head),
                    index,
                    membership,
                    complete_roots,
                };
                for (key, name) in [
                    (head, Namespace::Head(id)),
                    (index, Namespace::Forbidden),
                    (plus(index, ONE), Namespace::Forbidden),
                ] {
                    require(names.insert((role.account.clone(), key), name).is_none(), "ambiguous enumerable-set namespace")?;
                }
                if let Some(base) = membership {
                    for key in [base, plus(base, ONE), plus(plus(base, ONE), ONE)] {
                        // Only the selected Securities initializer's fixed
                        // ISSUER admin scalar is a separate metadata capability.
                        // It stays protected from operation aliases and remains
                        // a store barrier; it is never a membership stage.
                        if Some(key) == set.semantics.fixed_admin_slot() {
                            continue;
                        }
                        require(
                            names.insert((role.account.clone(), key), Namespace::Forbidden).is_none(),
                            "ambiguous membership namespace",
                        )?;
                    }
                    for (key, member) in by_parent.get(&base).into_iter().flatten() {
                        let name = if member[..12] == [0; 12] {
                            Namespace::Membership(id, *member)
                        } else {
                            Namespace::Forbidden
                        };
                        require(names.insert((role.account.clone(), *key), name).is_none(), "ambiguous membership leaf")?;
                    }
                }
                for (key, member) in by_parent.get(&index).into_iter().flatten() {
                    let name = if member[..12] == [0; 12] {
                        Namespace::Index(id, *member)
                    } else {
                        Namespace::Forbidden
                    };
                    require(
                        names.insert((role.account.clone(), *key), name).is_none(),
                        "ambiguous enumerable-set index namespace",
                    )?;
                }
                roles.push(role);
            }
        }
    }
    Ok(Namespaces { roles, names, protected })
}

fn member(event: &Event, role: usize, names: &BTreeMap<Key, Namespace>) -> Option<Word> {
    match names.get(&(event.account.clone(), event.slot.key)) {
        Some(Namespace::Index(owner, address)) if *owner == role => Some(*address),
        _ => None,
    }
}

// The tiny matcher can omit only independently derived equal assignments. Two
// embeddings of the same observed zero no-op are equivalent, not ambiguous.
fn matches(logical: &[Stage], observed: &[usize], events: &[Event]) -> bool {
    match logical.split_first() {
        None => observed.is_empty(),
        Some((first, rest)) => {
            (first.slot.old == first.slot.new && matches(rest, observed, events))
                || observed
                    .split_first()
                    .is_some_and(|(id, remaining)| events[*id].slot == first.slot && matches(rest, remaining, events))
        }
    }
}

fn legacy_candidates(
    role_id: usize,
    head: usize,
    position: usize,
    sequence: &[usize],
    events: &[Event],
    roles: &[Role],
    names: &BTreeMap<Key, Namespace>,
) -> Vec<Operation> {
    let role = &roles[role_id];
    let header = events[head].slot;
    let mut found = Vec::new();
    let mut insert = |logical: Vec<Stage>, start: usize, end: usize| {
        let observed = &sequence[start..=end];
        if matches(&logical, observed, events) {
            let candidate = Operation {
                role: role_id,
                head,
                observed: observed.to_vec(),
                logical,
                constraints: vec![],
            };
            if !found.contains(&candidate) {
                found.push(candidate);
            }
        }
    };
    if increment(header.old) == Some(header.new) {
        // Add: length, appended element, new one-based membership index.
        for end in position + 1..=(position + 2).min(sequence.len().saturating_sub(1)) {
            let last = &events[sequence[end]];
            let Some(address) = member(last, role_id, names) else { continue };
            if last.slot.old != ZERO || last.slot.new != header.new {
                continue;
            }
            insert(
                stages(
                    [
                        header,
                        Slot {
                            key: plus(role.array, header.old),
                            old: ZERO,
                            new: address,
                        },
                        last.slot,
                    ],
                    [StageKind::Length, StageKind::Element, StageKind::Position],
                ),
                position,
                end,
            );
        }
    } else if decrement(header.old) == Some(header.new) {
        // Remove: destination, moved index, cleared tail, length, deleted index.
        let Some(&last_id) = sequence.get(position + 1) else { return found };
        let last = &events[last_id];
        let Some(address) = member(last, role_id, names) else { return found };
        let p = last.slot.old;
        if p == ZERO || p > header.old || last.slot.new != ZERO {
            return found;
        }
        let mut tails = BTreeSet::new();
        if p == header.old {
            tails.insert(address);
        } else {
            for &id in &sequence[position.saturating_sub(3)..position] {
                let moved = &events[id];
                if moved.slot.old == header.old && moved.slot.new == p {
                    if let Some(tail) = member(moved, role_id, names) {
                        if tail != address {
                            tails.insert(tail);
                        }
                    }
                }
            }
        }
        for tail in tails {
            let logical = stages(
                [
                    Slot {
                        key: plus(role.array, decrement(p).unwrap()),
                        old: address,
                        new: tail,
                    },
                    Slot {
                        key: mapping(tail, role.index),
                        old: header.old,
                        new: p,
                    },
                    Slot {
                        key: plus(role.array, header.new),
                        old: tail,
                        new: ZERO,
                    },
                    header,
                    last.slot,
                ],
                [
                    StageKind::Element,
                    StageKind::Position,
                    StageKind::Element,
                    StageKind::Length,
                    StageKind::Position,
                ],
            );
            for start in position.saturating_sub(3)..=position {
                insert(logical.clone(), start, position + 1);
            }
        }
    }
    // Equality prefixes can produce nested windows for the same operation. The
    // maximal matching window consumes every visible source-implied no-op. A
    // prior operation's changing final index can never be such an optional prefix.
    let all = found.clone();
    found.retain(|candidate| {
        !all.iter().any(|other| {
            other.logical == candidate.logical
                && other.observed.len() > candidate.observed.len()
                && candidate.observed.iter().all(|id| other.observed.contains(id))
        })
    });
    found
}

// Independently proven selected PToken, Securities and coherent GM templates.
// Membership precedes every set mutation. No standalone bool/index/length
// fragment can supply permission.
fn coupled_candidates(
    role_id: usize,
    head: usize,
    position: usize,
    sequence: &[usize],
    events: &[Event],
    roles: &[Role],
    names: &BTreeMap<Key, Namespace>,
) -> Vec<Operation> {
    let role = &roles[role_id];
    if !role.complete_roots {
        return vec![];
    }
    let base = role.membership.unwrap();
    let header = events[head].slot;
    let mut found = vec![];
    let membership = |address: Word| {
        let key = mapping(address, base);
        matches!(names.get(&(role.account.clone(),key)),Some(Namespace::Membership(owner,member)) if *owner==role_id && *member==address).then_some(key)
    };
    let mut insert = |logical: Vec<Stage>, constraints: Vec<Slot>, start: usize, end: usize| {
        let observed = &sequence[start..=end];
        if matches(&logical, observed, events) {
            let op = Operation {
                role: role_id,
                head,
                observed: observed.to_vec(),
                logical,
                constraints,
            };
            if !found.contains(&op) {
                found.push(op);
            }
        }
    };
    if increment(header.old) == Some(header.new) {
        let Some(start) = position.checked_sub(1) else { return found };
        for end in position + 1..=(position + 2).min(sequence.len().saturating_sub(1)) {
            let last = &events[sequence[end]];
            let Some(address) = member(last, role_id, names) else { continue };
            let Some(key) = membership(address) else { continue };
            if last.slot.old != ZERO || last.slot.new != header.new {
                continue;
            }
            insert(
                stages(
                    [
                        Slot { key, old: ZERO, new: ONE },
                        header,
                        Slot {
                            key: plus(role.array, header.old),
                            old: ZERO,
                            new: address,
                        },
                        last.slot,
                    ],
                    [StageKind::Membership, StageKind::Length, StageKind::Element, StageKind::Position],
                ),
                vec![],
                start,
                end,
            );
        }
    } else if decrement(header.old) == Some(header.new) {
        let Some(&last_id) = sequence.get(position + 1) else { return found };
        let last = &events[last_id];
        let Some(address) = member(last, role_id, names) else { return found };
        let Some(key) = membership(address) else { return found };
        let p = last.slot.old;
        if p == ZERO || p > header.old || last.slot.new != ZERO {
            return found;
        }
        let boolean = Slot { key, old: ONE, new: ZERO };
        if p == header.old {
            let logical = stages(
                [
                    boolean,
                    Slot {
                        key: plus(role.array, header.new),
                        old: address,
                        new: ZERO,
                    },
                    header,
                    last.slot,
                ],
                [StageKind::Membership, StageKind::Element, StageKind::Length, StageKind::Position],
            );
            for start in position.saturating_sub(2)..position {
                insert(logical.clone(), vec![], start, position + 1);
            }
        } else {
            let mut tails = BTreeSet::new();
            for &id in &sequence[position.saturating_sub(4)..position] {
                let moved = &events[id];
                if moved.slot.old == header.old && moved.slot.new == p {
                    if let Some(tail) = member(moved, role_id, names) {
                        if tail != address {
                            tails.insert(tail);
                        }
                    }
                }
            }
            for tail in tails {
                let logical = stages(
                    [
                        boolean,
                        Slot {
                            key: plus(role.array, decrement(p).unwrap()),
                            old: address,
                            new: tail,
                        },
                        Slot {
                            key: mapping(tail, role.index),
                            old: header.old,
                            new: p,
                        },
                        Slot {
                            key: plus(role.array, header.new),
                            old: tail,
                            new: ZERO,
                        },
                        header,
                        last.slot,
                    ],
                    [
                        StageKind::Membership,
                        StageKind::Element,
                        StageKind::Position,
                        StageKind::Element,
                        StageKind::Length,
                        StageKind::Position,
                    ],
                );
                // The source never reads or writes M(t). Derive its invariant
                // only as a coherence constraint; require no invented preimage.
                let constraints = vec![Slot {
                    key: mapping(tail, base),
                    old: ONE,
                    new: ONE,
                }];
                for start in position.saturating_sub(4)..position {
                    insert(logical.clone(), constraints.clone(), start, position + 1);
                }
            }
        }
    }
    found
}

fn scopes(operations: &[Operation], collected: &Collected) -> Result<(), Error> {
    if operations.is_empty() {
        return Ok(());
    }
    let Collected {
        events,
        frames,
        storage_ordinals,
        storage_frames,
    } = collected;
    let groups: BTreeSet<_> = operations.iter().map(|op| frames[events[op.observed[0]].frame].group).collect();
    let mut grouped = BTreeMap::<usize, Vec<usize>>::new();
    let mut boundaries = Vec::with_capacity(frames.len() * 2);
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for (id, frame) in frames.iter().enumerate() {
        if groups.contains(&frame.group) {
            grouped.entry(frame.group).or_default().push(id);
        }
        // Foreign groups do not supply witnesses. An unrelated malformed frame
        // is not itself a rejection, but every known boundary remains a barrier.
        boundaries.extend([frame.begin, frame.end].into_iter().filter(|ordinal| *ordinal > 0));
        if frame.begin > 0 && frame.end > frame.begin {
            starts.push(id);
            ends.push(id);
        }
    }
    boundaries.sort_unstable();
    starts.sort_unstable_by_key(|id| frames[*id].begin);
    ends.sort_unstable_by_key(|id| frames[*id].end);
    let mut children = vec![Vec::<usize>::new(); frames.len()];
    for ids in grouped.values_mut() {
        ids.sort_unstable_by_key(|id| (frames[*id].begin, std::cmp::Reverse(frames[*id].end)));
        let mut stack = Vec::<usize>::new();
        for &id in ids.iter() {
            let frame = frames[id];
            require(
                frame.begin > 0 && frame.end > frame.begin,
                "enumerable-set operation has an ambiguous execution frame",
            )?;
            while stack.last().is_some_and(|parent| frames[*parent].end <= frame.begin) {
                stack.pop();
            }
            if let Some(&parent) = stack.last() {
                require(
                    frames[parent].begin < frame.begin && frame.end < frames[parent].end,
                    "enumerable-set execution frames overlap ambiguously",
                )?;
                children[parent].push(id);
            }
            stack.push(id);
        }
    }
    let mut barrier_ordinals = BTreeSet::new();
    for &(frame_id, ordinal) in storage_frames.iter().filter(|(frame_id, _)| groups.contains(&frames[*frame_id].group)) {
        let frame = frames[frame_id];
        require(
            ordinal > 0 && frame.begin < ordinal && ordinal < frame.end && barrier_ordinals.insert(ordinal),
            "enumerable-set storage barrier has an ambiguous ordinal or frame",
        )?;
        let nested = &children[frame_id];
        let before = nested.partition_point(|child| frames[*child].begin <= ordinal);
        require(
            before == 0 || frames[nested[before - 1]].end < ordinal,
            "enumerable-set storage belongs to a nested execution frame",
        )?;
    }
    let mut ordered = operations.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by_key(|op| events[op.observed[0]].ordinal);
    let mut active_groups = BTreeMap::<usize, usize>::new();
    let (mut start, mut end) = (0, 0);
    for operation in ordered {
        let first = &events[operation.observed[0]];
        let last = &events[*operation.observed.last().unwrap()];
        // Sweep valid intervals, including foreign transactions/system calls.
        // A foreign frame enclosing the complete operation has no boundary in
        // the window, but is still an impossible interleaving and must reject.
        while start < starts.len() && frames[starts[start]].begin <= first.ordinal {
            *active_groups.entry(frames[starts[start]].group).or_default() += 1;
            start += 1;
        }
        while end < ends.len() && frames[ends[end]].end < first.ordinal {
            let group = frames[ends[end]].group;
            let count = active_groups.get_mut(&group).unwrap();
            *count -= 1;
            if *count == 0 {
                active_groups.remove(&group);
            }
            end += 1;
        }
        require(
            active_groups.keys().all(|group| *group == frames[first.frame].group),
            "enumerable-set operation overlaps another execution group",
        )?;
        let boundary = boundaries.partition_point(|ordinal| *ordinal < first.ordinal);
        require(
            boundaries.get(boundary).is_none_or(|ordinal| *ordinal > last.ordinal),
            "enumerable-set operation crosses an execution-frame boundary",
        )?;
        let before = storage_ordinals.partition_point(|ordinal| *ordinal < first.ordinal);
        let through = storage_ordinals.partition_point(|ordinal| *ordinal <= last.ordinal);
        require(
            through - before == operation.observed.len(),
            "enumerable-set operation contains interleaved storage events",
        )?;
    }
    Ok(())
}

fn constrain(known: &mut BTreeMap<Key, Word>, account: &[u8], slot: Slot) -> Result<(), Error> {
    let key = (account.to_vec(), slot.key);
    if let Some(previous) = known.get(&key) {
        require(*previous == slot.old, "enumerable-set observed/inferred state is discontinuous")?;
    }
    known.insert(key, slot.new);
    Ok(())
}

pub(crate) fn validate(
    block: &eth::Block,
    layouts: &[VerifiedLayout],
    preimages: &BTreeMap<Word, Vec<u8>>,
    balance_candidates: &BTreeMap<Word, BTreeMap<Word, Vec<u8>>>,
) -> Result<Accepted, Error> {
    let accounts: BTreeSet<_> = layouts
        .iter()
        .filter(|l| !l.enumerable_address_sets.is_empty())
        .map(|l| l.contract.clone())
        .collect();
    if accounts.is_empty() {
        return Ok(BTreeSet::new());
    }
    if layouts
        .iter()
        .any(|l| l.enumerable_address_sets.iter().any(|s| s.semantics.membership_root().is_some()))
    {
        require(
            matches!(block.ver, 4 | 5),
            "selected coupled enumerable template requires Extended producer version4 or5",
        )?;
    }
    let coupled_accounts = layouts
        .iter()
        .filter(|l| l.enumerable_address_sets.iter().any(|s| s.semantics.membership_root().is_some()))
        .map(|l| l.contract.clone())
        .collect();
    let collected = collect(block, &accounts, &coupled_accounts)?;
    let events = &collected.events;
    let fixed_admin: BTreeSet<_> = layouts
        .iter()
        .flat_map(|layout| {
            layout
                .enumerable_address_sets
                .iter()
                .filter_map(move |set| set.semantics.fixed_admin_slot().map(|slot| (layout.contract.clone(), slot)))
        })
        .collect();
    for event in events {
        if fixed_admin.contains(&(event.account.clone(), event.slot.key)) {
            require(
                event.slot.new == ZERO,
                "selected Securities ISSUER admin write must set DEFAULT_ADMIN_ROLE zero",
            )?;
        }
    }
    let Namespaces { roles, names, protected } = namespaces(layouts, preimages, balance_candidates)?;
    let mut sequences = BTreeMap::<(usize, Vec<u8>), Vec<usize>>::new();
    for (id, event) in events.iter().enumerate() {
        sequences.entry((event.frame, event.account.clone())).or_default().push(id);
        require(
            !matches!(names.get(&(event.account.clone(), event.slot.key)), Some(Namespace::Forbidden)),
            "enumerable-set anchor/admin/dirty-key write",
        )?;
    }
    for sequence in sequences.values_mut() {
        sequence.sort_by_key(|id| events[*id].ordinal);
    }
    let mut operations = Vec::new();
    let mut consumed = BTreeSet::new();
    let mut operation_keys = BTreeMap::new();
    for sequence in sequences.values() {
        for (position, &id) in sequence.iter().enumerate() {
            let event = &events[id];
            if event.slot.old == event.slot.new {
                continue;
            }
            let Some(Namespace::Head(role)) = names.get(&(event.account.clone(), event.slot.key)) else {
                continue;
            };
            let mut possible = if roles[*role].membership.is_some() {
                coupled_candidates(*role, id, position, sequence, events, &roles, &names)
            } else {
                legacy_candidates(*role, id, position, sequence, events, &roles, &names)
            };
            require(possible.len() == 1, "enumerable-set length lacks a unique complete operation")?;
            let operation = possible.pop().unwrap();
            for &observed in &operation.observed {
                require(consumed.insert(observed), "enumerable-set storage witness reused")?;
                // Even a computational storage collision cannot let one role
                // consume another role's known head or membership namespace.
                if let Some(name) = names.get(&(event.account.clone(), events[observed].slot.key)) {
                    require(
                        matches!(name, Namespace::Head(owner) | Namespace::Index(owner, _) | Namespace::Membership(owner, _) if *owner == *role),
                        "enumerable-set operation aliases another namespace",
                    )?;
                }
            }
            for stage in &operation.logical {
                let logical = &stage.slot;
                let key = (event.account.clone(), logical.key);
                require(
                    !protected.contains(&key),
                    "enumerable-set operation aliases a protected root or known balance leaf",
                )?;
                // Inferred equal stores get the same alias checks as observed
                // stores. Array positions must not overlap any known role field,
                // even for the same role and even if the no-op was omitted.
                let element = stage.kind == StageKind::Element;
                if element {
                    require(!names.contains_key(&key), "enumerable-set element aliases a role namespace")?;
                }
                if let Some(owner) = operation_keys.insert(key, *role) {
                    require(owner == *role, "enumerable-set operations alias another role's storage")?;
                }
            }
            for constraint in &operation.constraints {
                let key = (event.account.clone(), constraint.key);
                require(!protected.contains(&key), "coherence key aliases protected storage")?;
                require(
                    operation.logical.iter().all(|s| s.slot.key != constraint.key),
                    "operation writes its moved-member coherence key",
                )?;
                if let Some(name) = names.get(&key) {
                    require(
                        matches!(name,Namespace::Membership(owner,_) if *owner==*role),
                        "coherence key aliases another namespace",
                    )?;
                }
                if let Some(owner) = operation_keys.insert(key, *role) {
                    require(owner == *role, "coherence key aliases another role")?;
                }
            }
            operations.push(operation);
        }
    }
    let coherence_keys: BTreeSet<_> = operations
        .iter()
        .flat_map(|op| {
            let account = &roles[op.role].account;
            op.constraints.iter().map(move |slot| (account.clone(), slot.key))
        })
        .collect();
    for operation in &operations {
        for stage in &operation.logical {
            if stage.kind == StageKind::Element {
                require(
                    !coherence_keys.contains(&(roles[operation.role].account.clone(), stage.slot.key)),
                    "array element aliases a derived membership coherence key",
                )?;
            }
        }
    }
    for (id, event) in events.iter().enumerate() {
        let key = (event.account.clone(), event.slot.key);
        if names.contains_key(&key) || operation_keys.contains_key(&key) {
            require(consumed.contains(&id), "unconsumed enumerable-set storage event")?;
        }
    }
    scopes(&operations, &collected)?;
    let starts: BTreeMap<_, _> = operations.iter().map(|op| (op.observed[0], op)).collect();
    let mut chronological = (0..events.len()).collect::<Vec<_>>();
    chronological.sort_by_key(|id| events[*id].ordinal);
    let mut known = BTreeMap::new();
    let mut accepted = BTreeSet::new();
    for id in chronological {
        let event = &events[id];
        if let Some(operation) = starts.get(&id) {
            for &constraint in &operation.constraints {
                constrain(&mut known, &event.account, constraint)?;
            }
            for stage in &operation.logical {
                constrain(&mut known, &event.account, stage.slot)?;
            }
        } else if !consumed.contains(&id) {
            constrain(&mut known, &event.account, event.slot)?;
        }
        if consumed.contains(&id) {
            accepted.insert((event.account.clone(), event.slot.key, event.ordinal));
        }
    }
    Ok(accepted)
}
