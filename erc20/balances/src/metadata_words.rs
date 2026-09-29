//! Closed metadata contracts for two captured direct getters. These validate
//! field/value domains, not authorization, external execution or global state.
use crate::{hash, hex_bytes, layout::Layout, require, subtract_offset, word};
use std::collections::{BTreeMap, BTreeSet};
use substreams::errors::Error;
use substreams_ethereum::pb::eth::v2 as eth;

pub const BNBTIGER: &str = "0xac68931b666e086e9de380cfdb0fb5704a35dc2d";
pub const COOKIE: &str = "0x3505bee89d3b4e351dbd4849241a6b0716ea407f";
pub const BNBTIGER_CODE: &str = "0x18f0619e94b94d884d917414ff4248c11e327998aef6a3e7089b9977e6ecef1c";
pub const COOKIE_CODE: &str = "0x258d1c6c6dcac1bfb46886ed147f9445408e9e5420a2c7a9434cd1b60bcf200f";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataSemantics {
    Bnbtiger,
    Cookie,
}
pub(crate) fn parse(layout: &Layout) -> Result<Option<MetadataSemantics>, Error> {
    let Some(name) = &layout.metadata_semantics else { return Ok(None) };
    let (mode, contract, code, root) = match name.as_str() {
        "bnbtiger_solc_0_8_4" => (MetadataSemantics::Bnbtiger, BNBTIGER, BNBTIGER_CODE, 7),
        "cookie_solc_0_6_12" => (MetadataSemantics::Cookie, COOKIE, COOKIE_CODE, 1),
        _ => return Err(Error::msg("unsupported source-bound metadata semantics")),
    };
    require(
        hex_bytes(&layout.contract)? == hex_bytes(contract)?
            && hex_bytes(&layout.code_hash)? == hex_bytes(code)?
            && hex_bytes(&layout.balance_slot)? == number(root)
            && layout.balance_bits.is_none(),
        "metadata template contract/runtime/balance identity mismatch",
    )?;
    require(
        layout.other_slots.is_empty()
            && layout.other_mapping_slots.is_empty()
            && layout.other_mapping_words.is_empty()
            && layout.other_mapping_paths.is_empty()
            && layout.enumerable_address_sets.is_empty()
            && layout.voting_checkpoints.is_none()
            && layout.address_lists.is_empty()
            && layout.zero_balance.is_none()
            && layout.balance_divisor.is_none()
            && layout.proxy.is_none()
            && layout.beacon_proxy.is_none()
            && layout.minimal_proxy.is_none()
            && layout.address_hash_balance.is_none()
            && layout.deployment.is_none()
            && !layout.immutable_zero_mapping,
        "source-bound metadata template cannot combine with another permission or dependency",
    )?;
    Ok(Some(mode))
}
fn number(n: u8) -> [u8; 32] {
    let mut word = [0; 32];
    word[31] = n;
    word
}

#[derive(Clone, Copy)]
enum Bound {
    Any,
    Max(u64),
    Fixed(u64),
}
fn small(value: &[u8]) -> Option<u64> {
    if value.len() > 8 && value[..value.len() - 8].iter().any(|b| *b != 0) {
        return None;
    }
    Some(value.iter().rev().take(8).enumerate().fold(0, |n, (i, b)| n | (u64::from(*b) << (8 * i))))
}
fn field(word: &[u8; 32], offset: usize, size: usize) -> &[u8] {
    &word[32 - offset - size..32 - offset]
}
fn packed(old: &[u8; 32], new: &[u8; 32], fields: &[(usize, usize, Bound)]) -> Result<(), Error> {
    let mut used = [false; 32];
    let mut changed = 0;
    for &(offset, size, bound) in fields {
        used[32 - offset - size..32 - offset].fill(true);
        let a = field(old, offset, size);
        let b = field(new, offset, size);
        changed += usize::from(a != b);
        for value in [a, b] {
            require(
                match bound {
                    Bound::Any => true,
                    Bound::Max(max) => small(value).is_some_and(|n| n <= max),
                    Bound::Fixed(expected) => small(value) == Some(expected),
                },
                "metadata field violates source value constraint",
            )?;
        }
    }
    require(
        (0..32).all(|i| used[i] || (old[i] == 0 && new[i] == 0)),
        "metadata word has nonzero unused padding",
    )?;
    // Deliberately conservative: selected runtime setters assign packed fields
    // separately. This is not an assertion about every future compiler build.
    require(changed <= 1, "multiple packed metadata fields changed in one record")
}
#[derive(Clone, Copy)]
enum Rule {
    Address,
    Bool,
    Any,
    BnbWallet,
    BnbPair,
    CookieRates,
    CookiePair,
    Operator,
    Supply,
    FromBlock,
    Count,
    Nonce,
}
fn increment(old: &[u8], new: &[u8]) -> bool {
    let mut expected = old.to_vec();
    for byte in expected.iter_mut().rev() {
        let (next, carry) = byte.overflowing_add(1);
        *byte = next;
        if !carry {
            break;
        }
    }
    expected == new
}
fn check(rule: Rule, old: &[u8; 32], new: &[u8; 32], block: u64) -> Result<(), Error> {
    use Bound::{Any, Fixed, Max};
    match rule {
        Rule::Address => packed(old, new, &[(0, 20, Any)]),
        Rule::Bool => packed(old, new, &[(0, 1, Max(1))]),
        Rule::Any => Ok(()),
        Rule::BnbWallet => packed(old, new, &[(0, 1, Fixed(9)), (1, 20, Any)]),
        Rule::BnbPair => packed(old, new, &[(0, 20, Any), (20, 1, Max(1)), (21, 1, Max(1)), (22, 1, Max(1)), (23, 1, Max(1))]),
        Rule::CookieRates => packed(old, new, &[(0, 1, Fixed(18)), (1, 2, Max(1000)), (3, 2, Max(100)), (5, 2, Max(500))]),
        Rule::CookiePair => {
            packed(old, new, &[(0, 20, Any), (20, 1, Max(1)), (21, 2, Fixed(500))])?;
            require(
                field(old, 0, 20) == field(new, 0, 20) || field(new, 0, 20).iter().any(|b| *b != 0),
                "COOKIE pair update must be nonzero",
            )
        }
        Rule::Operator => {
            packed(old, new, &[(0, 20, Any)])?;
            require(new.iter().any(|b| *b != 0), "COOKIE operator must be nonzero")
        }
        Rule::Supply => require(new >= old, "COOKIE reachable mint cannot decrease supply"),
        Rule::FromBlock => {
            packed(old, new, &[(0, 4, Any)])?;
            require(
                block <= u64::from(u32::MAX) && small(&new[28..]) == Some(block),
                "COOKIE checkpoint fromBlock must equal uint32 block number",
            )
        }
        Rule::Count => {
            packed(old, new, &[(0, 4, Any)])?;
            require(
                old == new || increment(&old[28..], &new[28..]),
                "COOKIE checkpoint count must wrap-increment uint32",
            )
        }
        Rule::Nonce => require(old == new || increment(old, new), "COOKIE nonce must wrap-increment uint256"),
    }
}
/// Exact nesting and canonical key widths, independently checking every hash.
fn path(mut key: [u8; 32], preimages: &BTreeMap<[u8; 32], Vec<u8>>, root: u8, widths: &[usize], offset: u8) -> Option<Vec<[u8; 32]>> {
    key = subtract_offset(key, offset);
    let mut keys = Vec::new();
    for width in widths.iter().rev() {
        let input = preimages.get(&key)?;
        if input.len() != 64 || hash(input) != key || input[..32 - width].iter().any(|b| *b != 0) {
            return None;
        }
        keys.push(input[..32].try_into().ok()?);
        key = input[32..].try_into().ok()?;
    }
    if key != number(root) {
        return None;
    }
    keys.reverse();
    Some(keys)
}
fn rules(mode: MetadataSemantics, key: [u8; 32], preimages: &BTreeMap<[u8; 32], Vec<u8>>) -> Result<Vec<Rule>, Error> {
    let scalar = if key[..31].iter().all(|b| *b == 0) {
        match (mode, key[31]) {
            (MetadataSemantics::Bnbtiger, 0 | 1 | 6 | 29) | (MetadataSemantics::Cookie, 0 | 10) => Some(Rule::Address),
            (MetadataSemantics::Bnbtiger, 2 | 13..=24 | 26..=28) | (MetadataSemantics::Cookie, 9) => Some(Rule::Any),
            (MetadataSemantics::Bnbtiger, 5) => Some(Rule::BnbWallet),
            (MetadataSemantics::Bnbtiger, 30) => Some(Rule::BnbPair),
            (MetadataSemantics::Cookie, 3) => Some(Rule::Supply),
            (MetadataSemantics::Cookie, 6) => Some(Rule::CookieRates),
            (MetadataSemantics::Cookie, 8) => Some(Rule::Bool),
            (MetadataSemantics::Cookie, 11) => Some(Rule::CookiePair),
            (MetadataSemantics::Cookie, 13) => Some(Rule::Operator),
            _ => None,
        }
    } else {
        None
    };
    let mut matches = scalar.into_iter().collect::<Vec<_>>();
    let allowance = if mode == MetadataSemantics::Bnbtiger { 8 } else { 2 };
    if let Some(keys) = path(key, preimages, allowance, &[20, 20], 0) {
        require(keys.iter().all(|k| k.iter().any(|b| *b != 0)), "source allowance keys must be nonzero")?;
        matches.push(Rule::Any);
    }
    let bool_roots: &[u8] = if mode == MetadataSemantics::Bnbtiger { &[9, 10, 11, 12] } else { &[7, 12] };
    for root in bool_roots {
        if path(key, preimages, *root, &[20], 0).is_some() {
            matches.push(Rule::Bool);
        }
    }
    if mode == MetadataSemantics::Cookie {
        for (root, rule) in [(14, Rule::Address), (16, Rule::Count), (17, Rule::Nonce)] {
            if path(key, preimages, root, &[20], 0).is_some() {
                matches.push(rule);
            }
        }
        for (offset, rule) in [(0, Rule::FromBlock), (1, Rule::Any)] {
            if path(key, preimages, 15, &[20, 4], offset).is_some() {
                matches.push(rule);
            }
        }
    }
    Ok(matches)
}
type Events = BTreeSet<(Vec<u8>, [u8; 32], u64)>;
type Candidates = BTreeMap<[u8; 32], BTreeMap<[u8; 32], Vec<u8>>>;
pub(crate) fn validate(
    block: u64,
    layouts: &[crate::layout::VerifiedLayout],
    storage: &[eth::StorageChange],
    preimages: &BTreeMap<[u8; 32], Vec<u8>>,
    candidates: &Candidates,
) -> Result<Events, Error> {
    let mut accepted = BTreeSet::new();
    let mut prior = BTreeMap::<(Vec<u8>, [u8; 32]), ([u8; 32], u64)>::new();
    for c in storage {
        let Some(layout) = layouts.iter().find(|l| l.contract == c.address && l.metadata_semantics.is_some()) else {
            continue;
        };
        let key = word(&c.key)?;
        let old = word(&c.old_value)?;
        let new = word(&c.new_value)?;
        require(c.ordinal > 0, "guarded storage requires positive ordinal")?;
        if let Some((value, ordinal)) = prior.insert((c.address.clone(), key), (new, c.ordinal)) {
            require(c.ordinal > ordinal && old == value, "discontinuous or ambiguous same-key guarded storage")?;
        }
        let mode = layout.metadata_semantics.unwrap();
        let root = if mode == MetadataSemantics::Bnbtiger { 7 } else { 1 };
        let balance = path(key, preimages, root, &[20], 0).is_some() || candidates.get(&layout.balance_slot).is_some_and(|p| p.contains_key(&key));
        let matched = rules(mode, key, preimages)?;
        require(
            matched.len() + usize::from(balance) == 1,
            "unknown, constructor-only or aliased guarded storage",
        )?;
        if let Some(rule) = matched.first() {
            check(*rule, &old, &new, block)?;
            accepted.insert((c.address.clone(), key, c.ordinal));
        }
    }
    Ok(accepted)
}
