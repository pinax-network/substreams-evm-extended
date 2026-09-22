//! Pins the fixture's slots and bit ranges to the compiler storage layout of
//! the pinned Comet source (`docs/evidence/storage-layouts/comet@f766f515.json`,
//! `solc 0.8.15 --storage-layout`). A fixture edit that diverges from the
//! compiled layout, or a compiled slot the fixture neither decodes nor
//! reviews, fails here. Host-only evidence check; not part of the map.
#![cfg(not(target_arch = "wasm32"))]
use compound_v3_balance_state::{keccak, parse};
use serde_json::Value;
use std::collections::BTreeSet;

const LAYOUT: &str = include_str!("../../../docs/evidence/storage-layouts/comet@f766f515.json");
const FIXTURE: &str = include_str!("fixtures/mainnet-cusdcv3-epoch.json");

struct Var {
    label: String,
    slot: u64,
    bit_offset: u32,
    bit_width: u32,
}
fn layout(contract: &str) -> Vec<Var> {
    let doc: Value = serde_json::from_str(LAYOUT).unwrap();
    let contracts = doc["contracts"].as_object().unwrap();
    let (_, l) = contracts
        .iter()
        .find(|(k, _)| k.ends_with(&format!(":{contract}")))
        .unwrap_or_else(|| panic!("{contract} not in layout"));
    l["storage"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let bytes: u32 = l["types"][s["type"].as_str().unwrap()]["numberOfBytes"].as_str().unwrap().parse().unwrap();
            Var {
                label: s["label"].as_str().unwrap().to_string(),
                slot: s["slot"].as_str().unwrap().parse().unwrap(),
                bit_offset: s["offset"].as_u64().unwrap() as u32 * 8,
                bit_width: bytes * 8,
            }
        })
        .collect()
}
fn word(slot: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&slot.to_be_bytes());
    w
}
fn var<'a>(vars: &'a [Var], label: &str) -> &'a Var {
    vars.iter().find(|v| v.label == label).unwrap_or_else(|| panic!("{label} missing"))
}

#[test]
fn fixture_slots_and_bit_ranges_match_the_compiled_comet_layout() {
    let cfg = parse(FIXTURE).unwrap();
    let m = &cfg.markets[0];
    let vars = layout("CometWithExtendedAssetList");
    // Indices word: baseSupplyIndex 0..64, baseBorrowIndex 64..128 (the map hardcodes these ranges).
    let supply = var(&vars, "baseSupplyIndex");
    let borrow = var(&vars, "baseBorrowIndex");
    assert_eq!((word(supply.slot), supply.bit_offset, supply.bit_width), (m.indices_slot, 0, 64));
    assert_eq!((word(borrow.slot), borrow.bit_offset, borrow.bit_width), (m.indices_slot, 64, 64));
    assert_eq!(var(&vars, "trackingSupplyIndex").bit_offset, 128);
    assert_eq!(var(&vars, "trackingBorrowIndex").bit_offset, 192);
    // Totals word: totalSupplyBase 0..104, totalBorrowBase 104..208, lastAccrualTime 208..248, pauseFlags 248..256.
    for (label, offset, width) in [
        ("totalSupplyBase", 0, 104),
        ("totalBorrowBase", 104, 104),
        ("lastAccrualTime", 208, 40),
        ("pauseFlags", 248, 8),
    ] {
        let v = var(&vars, label);
        assert_eq!((word(v.slot), v.bit_offset, v.bit_width), (m.totals_slot, offset, width), "{label}");
    }
    // Holder mapping and the reviewed mappings.
    assert_eq!(word(var(&vars, "userBasic").slot), m.user_basic_slot);
    let reviewed: BTreeSet<[u8; 32]> = ["totalsCollateral", "isAllowed", "userNonce", "userCollateral", "liquidatorPoints"]
        .iter()
        .map(|l| word(var(&vars, l).slot))
        .collect();
    assert_eq!(reviewed, m.other_mapping_slots.iter().copied().collect());
    // Completeness: every compiled slot is decoded or reviewed, and the only
    // extra reviewed scalar is the unstructured reentrancy guard.
    let compiled: BTreeSet<[u8; 32]> = vars.iter().map(|v| word(v.slot)).collect();
    let mut covered: BTreeSet<[u8; 32]> = [m.indices_slot, m.totals_slot, m.user_basic_slot].into();
    covered.extend(m.other_mapping_slots.iter().copied());
    assert_eq!(compiled, covered);
    assert_eq!(m.other_slots, vec![keccak(b"comet.reentrancy.guard")]);
    // UserBasic.principal is the low 104 bits of the mapping value struct.
    let doc: Value = serde_json::from_str(LAYOUT).unwrap();
    let (_, l) = doc["contracts"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(k, _)| k.ends_with(":CometWithExtendedAssetList"))
        .unwrap();
    let user_basic_type = l["storage"].as_array().unwrap().iter().find(|s| s["label"] == "userBasic").unwrap()["type"]
        .as_str()
        .unwrap()
        .to_string();
    let value_type = l["types"][&user_basic_type]["value"].as_str().unwrap();
    let members = l["types"][value_type]["members"].as_array().unwrap();
    let principal = members.iter().find(|x| x["label"] == "principal").unwrap();
    assert_eq!(
        (
            principal["slot"].as_str().unwrap(),
            principal["offset"].as_u64().unwrap(),
            principal["type"].as_str().unwrap()
        ),
        ("0", 0, "t_int104")
    );
    assert_eq!(l["types"][value_type]["numberOfBytes"].as_str().unwrap(), "32");
}
