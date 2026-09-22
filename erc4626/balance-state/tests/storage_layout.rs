//! Pins the vault fixtures to the compiler storage layouts of their pinned
//! sources (`docs/evidence/storage-layouts/`): StaticATokenLM (solc 0.8.20),
//! SavingsDai (0.8.17), Pot (0.6.12) and the OpenZeppelin ERC-7201 namespace
//! constants (v5.0.0). Host-only evidence check; not part of the map.
#![cfg(not(target_arch = "wasm32"))]
use erc4626_balance_state::{add_offset, keccak, parse, Model};
use serde_json::Value;
use std::collections::BTreeSet;

const STATA: &str = include_str!("../../../docs/evidence/storage-layouts/static-a-token-v3@101f5d97.json");
const SDAI: &str = include_str!("../../../docs/evidence/storage-layouts/sdai@66587976.json");
const POT: &str = include_str!("../../../docs/evidence/storage-layouts/dss-pot@fa4f6630.json");
const OZ: &str = include_str!("../../../docs/evidence/storage-layouts/openzeppelin-upgradeable@v5.0.0.json");
const FIAT_TOKEN: &str = include_str!("../../../docs/evidence/storage-layouts/fiat-token@v2.2.0.json");
const BSC: &str = include_str!("fixtures/bsc-stata-usdt-epoch.json");
const MAINNET: &str = include_str!("fixtures/mainnet-sdai-and-oz-epochs.json");

fn layout(doc: &str, contract: &str) -> Value {
    let doc: Value = serde_json::from_str(doc).unwrap();
    let (_, l) = doc["contracts"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(k, _)| k.ends_with(&format!(":{contract}")))
        .unwrap_or_else(|| panic!("{contract} not in layout"));
    l.clone()
}
fn slots(doc: &str, contract: &str) -> Vec<(String, u64)> {
    layout(doc, contract)["storage"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["label"].as_str().unwrap().to_string(), s["slot"].as_str().unwrap().parse().unwrap()))
        .collect()
}
/// Slots whose data lives at `keccak256(slot) + i`: dynamic arrays and
/// `bytes`/`string` (long values spill out of the root slot).
fn dynamic_slots(doc: &str, contract: &str) -> BTreeSet<[u8; 32]> {
    let l = layout(doc, contract);
    l["storage"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| matches!(l["types"][s["type"].as_str().unwrap()]["encoding"].as_str(), Some("bytes" | "dynamic_array")))
        .map(|s| word(s["slot"].as_str().unwrap().parse().unwrap()))
        .collect()
}
/// ERC-7201: `keccak256(abi.encode(uint256(keccak256(id)) - 1)) & ~bytes32(uint256(0xff))`.
fn erc7201(id: &str) -> [u8; 32] {
    let mut h = keccak(id.as_bytes());
    for byte in h.iter_mut().rev() {
        if *byte == 0 {
            *byte = 0xff;
        } else {
            *byte -= 1;
            break;
        }
    }
    let mut out = keccak(&h);
    out[31] = 0;
    out
}
fn word(slot: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&slot.to_be_bytes());
    w
}
fn slot_of(vars: &[(String, u64)], label: &str) -> [u8; 32] {
    word(vars.iter().find(|(l, _)| l == label).unwrap_or_else(|| panic!("{label} missing")).1)
}

#[test]
fn static_atoken_fixture_matches_the_compiled_layout() {
    let v = parse(BSC).unwrap().vaults[0].clone();
    let vars = slots(STATA, "StaticATokenLM");
    assert_eq!(v.balances_slot, slot_of(&vars, "balanceOf"));
    assert_eq!(v.total_supply_slot, slot_of(&vars, "totalSupply"));
    let mappings: BTreeSet<[u8; 32]> = ["allowance", "nonces", "_startIndex", "_userRewardsData"]
        .iter()
        .map(|l| slot_of(&vars, l))
        .collect();
    assert_eq!(mappings, v.other_mapping_slots.iter().copied().collect());
    let scalars: BTreeSet<[u8; 32]> = ["_initialized", "name", "symbol", "decimals", "_rewardTokens"]
        .iter()
        .map(|l| slot_of(&vars, l))
        .collect();
    assert_eq!(scalars, v.other_slots.iter().copied().collect());
    // `_aToken` and `_aTokenUnderlying` select the reserve and are pointers.
    let Model::AaveStaticAToken {
        atoken_slot, underlying_slot, ..
    } = v.model
    else {
        panic!()
    };
    assert_eq!((atoken_slot, underlying_slot), (slot_of(&vars, "_aToken"), slot_of(&vars, "_aTokenUnderlying")));
    // Every dynamic area (`name`, `symbol`, `_rewardTokens`) is reviewed as such.
    assert_eq!(dynamic_slots(STATA, "StaticATokenLM"), v.other_dynamic_slots.iter().copied().collect());
    let compiled: BTreeSet<[u8; 32]> = vars.iter().map(|(_, s)| word(*s)).collect();
    let mut covered: BTreeSet<[u8; 32]> = [v.balances_slot, v.total_supply_slot, atoken_slot, underlying_slot].into();
    covered.extend(v.other_slots.iter().copied());
    covered.extend(v.other_mapping_slots.iter().copied());
    assert_eq!(compiled, covered);
    // Initializable packs its two fields into slot 0.
    assert_eq!(
        vars.iter().filter(|(_, s)| *s == 0).map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
        vec!["_initialized", "_initializing"]
    );
}

#[test]
fn savings_dai_and_pot_fixtures_match_the_compiled_layouts() {
    let cfg = parse(MAINNET).unwrap();
    let sdai = cfg.vaults[0].clone();
    let vars = slots(SDAI, "SavingsDai");
    assert_eq!(sdai.balances_slot, slot_of(&vars, "balanceOf"));
    assert_eq!(sdai.total_supply_slot, slot_of(&vars, "totalSupply"));
    let mappings: BTreeSet<[u8; 32]> = ["allowance", "nonces"].iter().map(|l| slot_of(&vars, l)).collect();
    assert_eq!(mappings, sdai.other_mapping_slots.iter().copied().collect());
    assert_eq!(vars.len(), 4);
    assert!(dynamic_slots(SDAI, "SavingsDai").is_empty() && sdai.other_dynamic_slots.is_empty());
    let Model::MakerSavingsDai {
        dsr_slot, chi_slot, rho_slot, ..
    } = sdai.model
    else {
        panic!()
    };
    let pot = slots(POT, "Pot");
    assert_eq!(
        (dsr_slot, chi_slot, rho_slot),
        (slot_of(&pot, "dsr"), slot_of(&pot, "chi"), slot_of(&pot, "rho"))
    );
    assert_eq!(
        pot.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
        vec!["wards", "pie", "Pie", "dsr", "chi", "vat", "vow", "rho", "live"]
    );
}

#[test]
fn openzeppelin_namespace_constants_match_the_fixture() {
    let cfg = parse(MAINNET).unwrap();
    let oz = cfg.vaults[1].clone();
    let doc: Value = serde_json::from_str(OZ).unwrap();
    let erc20 = doc["namespaces"]["openzeppelin.storage.ERC20"]["value"].as_str().unwrap();
    let base: [u8; 32] = hex::decode(&erc20[2..]).unwrap().try_into().unwrap();
    assert_eq!(oz.balances_slot, base);
    assert_eq!(oz.other_mapping_slots, vec![add_offset(&base, 1)]);
    assert_eq!(oz.total_supply_slot, add_offset(&base, 2));
    // The harness compiled without any regular storage: all state is namespaced.
    for (_, l) in doc["contracts"].as_object().unwrap() {
        assert!(l["storage"].as_array().unwrap().is_empty());
    }
    // Every namespace member must be decoded, reviewed or guarded: `_name`
    // (+3) and `_symbol` (+4) are written by `__ERC20_init_unchained`, and the
    // ERC4626 namespace word (`_asset`, `_underlyingDecimals`) by
    // `__ERC4626_init_unchained`, so an initializing block would otherwise be
    // refused as unresolved storage.
    let members = doc["namespaces"]["openzeppelin.storage.ERC20"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 5);
    let decoded = [oz.balances_slot, add_offset(&base, 1), oz.total_supply_slot];
    for (i, member) in members.iter().enumerate() {
        let slot = add_offset(&base, i as u8);
        let covered = decoded.contains(&slot) || oz.other_slots.contains(&slot) || oz.other_mapping_slots.contains(&slot);
        assert!(covered, "ERC20 namespace member {member} (+{i}) is neither decoded nor reviewed");
    }
    let erc4626 = doc["namespaces"]["openzeppelin.storage.ERC4626"]["value"].as_str().unwrap();
    let erc4626: [u8; 32] = hex::decode(&erc4626[2..]).unwrap().try_into().unwrap();
    let Model::OzVirtualOffset { erc4626_storage_slot, .. } = cfg.vaults[1].model.clone() else {
        panic!()
    };
    assert_eq!(erc4626_storage_slot, erc4626);
    assert_eq!(doc["namespaces"]["openzeppelin.storage.ERC4626"]["members"].as_array().unwrap().len(), 2);
    // `_name` and `_symbol` are strings: their data areas are reviewed too.
    assert_eq!(oz.other_dynamic_slots, vec![add_offset(&base, 3), add_offset(&base, 4)]);
    // The captured harness evidence lists two namespaces, but the compiled
    // `Initializable` of the same commit keeps `_initialized`/`_initializing`
    // in a third one, `INITIALIZABLE_STORAGE` (Initializable.sol:77), written
    // by every `initializer` and `reinitializer`.
    let initializable = erc7201("openzeppelin.storage.Initializable");
    assert_eq!(hex::encode(initializable), "f0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00");
    assert!(oz.other_slots.contains(&initializable));
    assert_eq!(erc7201("openzeppelin.storage.ERC20"), base);
    assert_eq!(erc7201("openzeppelin.storage.ERC4626"), erc4626);
}

#[test]
fn usdc_asset_layout_and_proxy_slot_match_the_pinned_source() {
    use erc4626_balance_state::{keccak, mapping_key, AssetBalanceModel};
    let cfg = parse(MAINNET).unwrap();
    let oz = &cfg.vaults[1];
    let vars = slots(FIAT_TOKEN, "FiatTokenV2_2");
    let Model::OzVirtualOffset {
        asset_balance_key,
        asset_balance_model,
        asset_source_pin,
        asset_implementation_slot,
        asset_implementation,
        ..
    } = &oz.model
    else {
        panic!()
    };
    assert_eq!(*asset_balance_key, mapping_key(&oz.vault, &slot_of(&vars, "balanceAndBlacklistStates")));
    assert_eq!(slot_of(&vars, "balanceAndBlacklistStates"), word(9));
    assert_eq!(*asset_balance_model, AssetBalanceModel::FiatTokenV2_2Low255);
    assert_eq!(asset_balance_model.value_bits(), 255);
    assert_eq!(*asset_implementation_slot, Some(keccak(b"org.zeppelinos.proxy.implementation")));
    assert!(asset_implementation.is_some());
    assert!(asset_source_pin.contains("405efc100c016ed1a437063b6274b4e24ea7b8b1"));
    assert!(asset_source_pin.contains("placeholder implementation"));
}
