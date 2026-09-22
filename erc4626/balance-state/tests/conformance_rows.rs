//! Feeds the rows the map emits into `conformance::erc4626`, so a key, scale
//! or bit-range mismatch between the extraction and the reference model is a
//! test failure rather than a consumer surprise. Host-only.
#![cfg(not(target_arch = "wasm32"))]
use conformance::{aave::Reserve, erc4626};
use erc4626_balance_state::{add_offset, mapping_key, parse, project, Model};
use num_bigint::BigUint;
use proto::pb::evm::balance_state::v1 as pb;
use substreams_ethereum::pb::eth::v2 as eth;

const BSC: &str = include_str!("fixtures/bsc-stata-usdt-epoch.json");
const MAINNET: &str = include_str!("fixtures/mainnet-sdai-and-oz-epochs.json");
const NOW: i64 = 1_789_689_600;

fn block(number: u64, calls: Vec<eth::Call>) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp { seconds: NOW, nanos: 0 }),
            ..Default::default()
        }),
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            hash: vec![7; 32],
            index: 1,
            calls,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn word(v: &BigUint) -> Vec<u8> {
    let bytes = v.to_bytes_be();
    let mut out = vec![0u8; 32 - bytes.len()];
    out.extend(bytes);
    out
}
fn packed(low: &BigUint, high: &BigUint) -> Vec<u8> {
    word(&(high << 128u32 | low))
}
fn write(address: &[u8], key: [u8; 32], old: Vec<u8>, new: Vec<u8>, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        key: key.to_vec(),
        old_value: old,
        new_value: new,
        ordinal,
    }
}
fn shares_write(vault: &[u8], balances: &[u8; 32], holder: &[u8; 20], shares: &BigUint, ordinal: u64) -> (eth::StorageChange, (String, String)) {
    let key = mapping_key(holder, balances);
    let mut preimage = vec![0u8; 64];
    preimage[12..32].copy_from_slice(holder);
    preimage[32..].copy_from_slice(balances);
    (
        write(vault, key, word(&BigUint::from(0u8)), word(shares), ordinal),
        (hex::encode(key), hex::encode(preimage)),
    )
}
fn global(events: &pb::Events, market: &[u8], field: pb::StateField) -> BigUint {
    let row = events
        .global_state
        .iter()
        .find(|g| g.market == market && g.field == field as i32)
        .unwrap_or_else(|| panic!("no {field:?} row"));
    row.value.parse().unwrap()
}
fn basis(events: &pb::Events, market: &[u8], holder: &[u8]) -> BigUint {
    events
        .holder_basis
        .iter()
        .find(|h| h.market == market && h.holder == holder)
        .unwrap()
        .value
        .parse()
        .unwrap()
}
fn timestamp(events: &pb::Events) -> u64 {
    events.clocks[0].timestamp
}

#[test]
fn savings_dai_rows_evaluate_with_the_pot_model() {
    let cfg = parse(MAINNET).unwrap();
    let sdai = &cfg.vaults[0];
    let Model::MakerSavingsDai {
        pot,
        dsr_slot,
        chi_slot,
        rho_slot,
    } = &sdai.model
    else {
        panic!()
    };
    let ray = BigUint::from(10u8).pow(27);
    let chi = &ray + &ray / 20u8;
    let dsr = &ray + BigUint::from(1_547_125_957_863_212_448u64);
    let rho = BigUint::from((NOW - 3_600) as u64);
    let shares = BigUint::from(123_456_789_000_000_000_000u128);
    let holder = [9u8; 20];
    let (share, preimage) = shares_write(&sdai.vault, &sdai.balances_slot, &holder, &shares, 13);
    let zero = word(&BigUint::from(0u8));
    let events = project(
        &block(
            10,
            vec![eth::Call {
                address: sdai.vault.clone(),
                keccak_preimages: [preimage].into(),
                storage_changes: vec![
                    write(pot, *chi_slot, zero.clone(), word(&chi), 10),
                    write(pot, *rho_slot, zero.clone(), word(&rho), 11),
                    write(pot, *dsr_slot, zero, word(&dsr), 12),
                    share,
                ],
                ..Default::default()
            }],
        ),
        &cfg,
    )
    .unwrap();
    let model = erc4626::SavingsDai {
        chi: global(&events, &sdai.vault, pb::StateField::MakerPotChi),
        rho: u64::try_from(global(&events, &sdai.vault, pb::StateField::MakerPotRho)).unwrap(),
        dsr: global(&events, &sdai.vault, pb::StateField::MakerPotDsr),
    };
    assert_eq!((&model.chi, &model.dsr), (&chi, &dsr));
    let assets = model.convert_to_assets(&basis(&events, &sdai.vault, &holder), timestamp(&events)).unwrap();
    // An hour of drip at this rate moves chi; the result is shares × chi′ / RAY.
    let chi_now = erc4626::rpow(&dsr, 3_600).unwrap() * &chi / &ray;
    assert_eq!(assets, &shares * chi_now / &ray);
}

#[test]
fn static_atoken_rows_evaluate_with_the_reserve_model() {
    let cfg = parse(BSC).unwrap();
    let stata = &cfg.vaults[0];
    let Model::AaveStaticAToken { pool, reserve_base, .. } = &stata.model else {
        panic!()
    };
    let ray = BigUint::from(10u8).pow(27);
    let index = &ray + &ray / 1000u16;
    let rate = &ray * 3u8 / 100u8;
    let updated = BigUint::from((NOW - 86_400) as u64);
    let shares = BigUint::from(5_000_000_000_000_000_000u128);
    let holder = [8u8; 20];
    let (share, preimage) = shares_write(&stata.vault, &stata.balances_slot, &holder, &shares, 12);
    let zero = BigUint::from(0u8);
    let events = project(
        &block(
            10,
            vec![eth::Call {
                address: stata.vault.clone(),
                keccak_preimages: [preimage].into(),
                storage_changes: vec![
                    write(pool, add_offset(reserve_base, 1), packed(&zero, &zero), packed(&index, &rate), 10),
                    write(pool, add_offset(reserve_base, 3), packed(&zero, &zero), packed(&zero, &updated), 11),
                    share,
                ],
                ..Default::default()
            }],
        ),
        &cfg,
    )
    .unwrap();
    let reserve = Reserve {
        liquidity_index: global(&events, &stata.vault, pb::StateField::AaveLiquidityIndex),
        current_liquidity_rate: global(&events, &stata.vault, pb::StateField::AaveCurrentLiquidityRate),
        last_update_timestamp: u64::try_from(global(&events, &stata.vault, pb::StateField::AaveLastUpdateTimestamp)).unwrap(),
    };
    assert_eq!(
        (&reserve.liquidity_index, &reserve.current_liquidity_rate, reserve.last_update_timestamp),
        (&index, &rate, (NOW - 86_400) as u64)
    );
    let model = erc4626::StataTokenLm {
        reserve: reserve.clone(),
        reserve_active_and_unpaused: Some(true),
    };
    let now = timestamp(&events);
    let assets = model.convert_to_assets(&basis(&events, &stata.vault, &holder), now).unwrap();
    // rayMulRoundDown(shares, normalized income).
    assert_eq!(assets, &shares * reserve.normalized_income(now).unwrap() / &ray);
}

#[test]
fn openzeppelin_rows_evaluate_with_the_virtual_offset_model() {
    let cfg = parse(MAINNET).unwrap();
    let oz = &cfg.vaults[1];
    let Model::OzVirtualOffset { asset_balance_key, .. } = &oz.model else {
        panic!()
    };
    let assets_held = BigUint::from(1_005_000_000u64); // USDC, 6 decimals
    let supply = BigUint::from(1_000_000_000_000_000_000_000u128); // 18-decimal shares
    let shares = BigUint::from(250_000_000_000_000_000_000u128);
    let holder = [7u8; 20];
    let (share, preimage) = shares_write(&oz.vault, &oz.balances_slot, &holder, &shares, 12);
    let zero = word(&BigUint::from(0u8));
    // The block at the activation height also carries the offset declaration.
    let events = project(
        &block(
            1,
            vec![eth::Call {
                address: oz.vault.clone(),
                keccak_preimages: [preimage].into(),
                storage_changes: vec![
                    write(&oz.asset, *asset_balance_key, zero.clone(), word(&assets_held), 10),
                    write(&oz.vault, oz.total_supply_slot, zero, word(&supply), 11),
                    share,
                ],
                ..Default::default()
            }],
        ),
        &cfg,
    )
    .unwrap();
    let model = erc4626::OzVirtualOffset {
        total_assets: global(&events, &oz.vault, pb::StateField::Erc4626TotalAssets),
        total_supply: global(&events, &oz.vault, pb::StateField::Erc4626TotalSupply),
        decimals_offset: u8::try_from(global(&events, &oz.vault, pb::StateField::Erc4626DecimalsOffset)).unwrap(),
    };
    let assets = model.convert_to_assets(&basis(&events, &oz.vault, &holder)).unwrap();
    let virtual_shares = &supply + BigUint::from(10u8).pow(12);
    assert_eq!(assets, &shares * (&assets_held + 1u8) / virtual_shares);
}
