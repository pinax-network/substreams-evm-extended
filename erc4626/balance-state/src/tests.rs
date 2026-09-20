use super::*;

const BSC: &str = include_str!("../tests/fixtures/bsc-stata-usdt-epoch.json");
const MAINNET: &str = include_str!("../tests/fixtures/mainnet-sdai-and-oz-epochs.json");

fn bsc() -> Config {
    parse(BSC).unwrap()
}
fn mainnet() -> Config {
    parse(MAINNET).unwrap()
}
fn block(number: u64) -> eth::Block {
    eth::Block {
        ver: 5,
        number,
        hash: vec![1; 32],
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        header: Some(eth::BlockHeader {
            number,
            parent_hash: vec![2; 32],
            state_root: vec![3; 32],
            timestamp: Some(prost_types::Timestamp { seconds: 1789689600, nanos: 0 }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn tx(call: eth::Call) -> eth::TransactionTrace {
    eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 9,
        calls: vec![call],
        ..Default::default()
    }
}
fn w(v: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[16..].copy_from_slice(&v.to_be_bytes());
    out
}
fn packed(low: u128, high: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(&high.to_be_bytes());
    out[16..].copy_from_slice(&low.to_be_bytes());
    out
}
fn write(address: &[u8], key: [u8; 32], old: [u8; 32], new: [u8; 32], ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: address.to_vec(),
        key: key.to_vec(),
        old_value: old.to_vec(),
        new_value: new.to_vec(),
        ordinal,
    }
}
fn preimage(holder: &[u8], base: &[u8; 32]) -> (String, String) {
    let mut p = vec![0u8; 64];
    p[12..32].copy_from_slice(holder);
    p[32..].copy_from_slice(base);
    (hex::encode(mapping_key(holder, base)), hex::encode(p))
}
fn shares_call(vault: &Vault, holder: &[u8], old: u128, new: u128, ordinal: u64) -> eth::Call {
    eth::Call {
        index: 1,
        address: vault.vault.clone(),
        keccak_preimages: [preimage(holder, &vault.balances_slot)].into(),
        storage_changes: vec![write(&vault.vault, mapping_key(holder, &vault.balances_slot), w(old), w(new), ordinal)],
        ..Default::default()
    }
}
fn fields(events: &pb::Events, market: &[u8]) -> Vec<(i32, String, String, i32)> {
    events
        .global_state
        .iter()
        .filter(|g| g.market == market)
        .map(|g| (g.field, g.previous_value.clone(), g.value.clone(), g.observation))
        .collect()
}

#[test]
fn shares_and_total_supply_are_the_vault_rows_for_every_model() {
    for (cfg, index) in [(bsc(), 0usize), (mainnet(), 0), (mainnet(), 1)] {
        let v = cfg.vaults[index].clone();
        let mut b = block(10);
        let mut call = shares_call(&v, &[9; 20], 100, 250, 10);
        call.storage_changes.push(write(&v.vault, v.total_supply_slot, w(1_000), w(1_150), 11));
        b.transaction_traces = vec![tx(call)];
        let events = project(&b, &cfg).unwrap();
        let mine: Vec<_> = events.holder_basis.iter().filter(|h| h.market == v.vault).collect();
        assert_eq!(mine.len(), 1);
        assert_eq!(
            (&*mine[0].previous_value, &*mine[0].value, mine[0].basis_kind),
            ("100", "250", pb::BasisKind::Shares as i32)
        );
        assert_eq!(
            fields(&events, &v.vault),
            vec![(pb::StateField::Erc4626TotalSupply as i32, "1000".into(), "1150".into(), 1)]
        );
        assert!(events.epochs.is_empty());
        // Unknown vault storage fails closed; reviewed mapping members do not.
        b.transaction_traces = vec![tx(eth::Call {
            address: v.vault.clone(),
            storage_changes: vec![write(&v.vault, w(0x777), w(0), w(1), 10)],
            ..Default::default()
        })];
        assert!(project(&b, &cfg).unwrap_err().to_string().contains("unresolved"));
        let base = v.other_mapping_slots[0];
        b.transaction_traces = vec![tx(eth::Call {
            address: v.vault.clone(),
            keccak_preimages: [preimage(&[4; 20], &base)].into(),
            storage_changes: vec![write(&v.vault, mapping_key(&[4; 20], &base), w(0), w(1), 10)],
            ..Default::default()
        })];
        assert!(project(&b, &cfg).unwrap().global_state.is_empty());
    }
}

#[test]
fn aave_model_carries_the_pool_reserve_words_of_the_asset_and_its_pointers() {
    let cfg = bsc();
    let v = cfg.vaults[0].clone();
    let Model::AaveStaticAToken {
        pool,
        reserve_base,
        implementation_slot,
        implementation,
        atoken,
    } = v.model.clone()
    else {
        panic!()
    };
    let mut b = block(10);
    let ray = 10u128.pow(27);
    let other_reserve = mapping_key(&[1; 20], &w(0x34));
    b.transaction_traces = vec![tx(eth::Call {
        address: pool.clone(),
        storage_changes: vec![
            write(
                &pool,
                add_offset(&reserve_base, 1),
                packed(ray, 3 * ray / 100),
                packed(ray + ray / 1000, 3 * ray / 100),
                10,
            ),
            write(&pool, add_offset(&reserve_base, 3), packed(0, 1_789_689_000), packed(0, 1_789_689_600), 11),
            write(&pool, add_offset(&reserve_base, 2), w(1), w(2), 12),
            write(&pool, add_offset(&other_reserve, 1), packed(ray, 0), packed(2 * ray, 0), 13),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events, &v.vault),
        vec![
            (pb::StateField::AaveLiquidityIndex as i32, ray.to_string(), (ray + ray / 1000).to_string(), 1),
            (pb::StateField::AaveLastUpdateTimestamp as i32, "1789689000".into(), "1789689600".into(), 1),
        ]
    );
    let index = &events.global_state[0];
    assert_eq!(
        (&index.key, &index.storage_contract, &*index.scale, index.bit_width),
        (&v.asset, &pool, RAY, 128)
    );
    assert_eq!((events.global_state[1].bit_offset, events.global_state[1].bit_width), (128, 40));
    // Pool implementation pointer write and dependency code changes invalidate.
    b.transaction_traces = vec![tx(eth::Call {
        address: pool.clone(),
        storage_changes: vec![write(&pool, implementation_slot.unwrap(), w(1), w(2), 10)],
        ..Default::default()
    })];
    assert_eq!(
        project(&b, &cfg).unwrap().epochs[0].reason,
        pb::InvalidationReason::DependencyPointerWrite as i32
    );
    for (address, reason) in [
        (v.vault.clone(), pb::InvalidationReason::CodeChange),
        (v.implementation.clone().unwrap(), pb::InvalidationReason::CodeChange),
        (pool.clone(), pb::InvalidationReason::DependencyCodeChange),
        (implementation.clone().unwrap(), pb::InvalidationReason::DependencyCodeChange),
        (atoken.clone(), pb::InvalidationReason::DependencyCodeChange),
        (v.asset.clone(), pb::InvalidationReason::DependencyCodeChange),
    ] {
        b.transaction_traces = vec![tx(eth::Call {
            code_changes: vec![eth::CodeChange {
                address,
                old_hash: vec![1; 32],
                new_hash: vec![2; 32],
                ordinal: 5,
                ..Default::default()
            }],
            ..Default::default()
        })];
        let events = project(&b, &cfg).unwrap();
        assert_eq!((events.epochs.len(), events.epochs[0].reason), (1, reason as i32));
    }
    b.transaction_traces = vec![tx(eth::Call {
        address: v.vault.clone(),
        storage_changes: vec![write(&v.vault, v.implementation_slot.unwrap(), w(1), w(2), 10)],
        ..Default::default()
    })];
    assert_eq!(
        project(&b, &cfg).unwrap().epochs[0].reason,
        pb::InvalidationReason::ImplementationPointerWrite as i32
    );
    // Binding rows.
    let events = project(&block(1), &cfg).unwrap();
    let roles: Vec<(u32, i32, i32)> = events.dependencies.iter().map(|d| (d.depth, d.role, d.binding)).collect();
    assert_eq!(
        roles,
        vec![
            (1, pb::DependencyRole::Implementation as i32, pb::BindingKind::StoragePointer as i32),
            (1, pb::DependencyRole::Pool as i32, pb::BindingKind::Declared as i32),
            (1, pb::DependencyRole::Underlying as i32, pb::BindingKind::Declared as i32),
            (1, pb::DependencyRole::WrappedAsset as i32, pb::BindingKind::Declared as i32),
            (2, pb::DependencyRole::Implementation as i32, pb::BindingKind::StoragePointer as i32),
        ]
    );
    assert_eq!(events.dependencies[4].parent, pool);
    let e = &events.epochs[0];
    assert_eq!(
        (e.family, e.basis_kind, &e.balance_asset, e.balance_decimals),
        (pb::ModelFamily::Erc4626Vault as i32, pb::BasisKind::Shares as i32, &v.vault, 18)
    );
    assert!(events.global_state.is_empty());
}

#[test]
fn sdai_model_carries_pot_dsr_chi_rho_and_oz_model_carries_asset_balance_and_offset() {
    let cfg = mainnet();
    let sdai = cfg.vaults[0].clone();
    let oz = cfg.vaults[1].clone();
    let Model::MakerSavingsDai {
        pot,
        dsr_slot,
        chi_slot,
        rho_slot,
    } = sdai.model.clone()
    else {
        panic!()
    };
    let Model::OzVirtualOffset {
        asset_balance_key,
        decimals_offset,
    } = oz.model.clone()
    else {
        panic!()
    };
    let ray = 10u128.pow(27);
    let mut b = block(10);
    // A Pot drip: chi and rho move, dsr does not; the Pie word (slot 2) is not an input.
    b.transaction_traces = vec![tx(eth::Call {
        address: pot.clone(),
        storage_changes: vec![
            write(&pot, chi_slot, w(ray + ray / 20), w(ray + ray / 19), 10),
            write(&pot, rho_slot, w(1_789_689_000), w(1_789_689_600), 11),
            write(&pot, w(2), w(5), w(6), 12),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events, &sdai.vault),
        vec![
            (
                pb::StateField::MakerPotChi as i32,
                (ray + ray / 20).to_string(),
                (ray + ray / 19).to_string(),
                1
            ),
            (pb::StateField::MakerPotRho as i32, "1789689000".into(), "1789689600".into(), 1),
        ]
    );
    assert_eq!((&*events.global_state[0].scale, &events.global_state[0].storage_contract), (RAY, &pot));
    b.transaction_traces = vec![tx(eth::Call {
        address: pot.clone(),
        storage_changes: vec![write(&pot, dsr_slot, w(ray), w(ray + 1_547_125_957_863_212_448), 10)],
        ..Default::default()
    })];
    assert_eq!(fields(&project(&b, &cfg).unwrap(), &sdai.vault)[0].0, pb::StateField::MakerPotDsr as i32);
    // A donation to the OZ vault: the asset's balance of the vault moves without any vault write.
    let asset = oz.asset.clone();
    let other = mapping_key(&[5; 20], &w(9));
    b.transaction_traces = vec![tx(eth::Call {
        address: asset.clone(),
        storage_changes: vec![
            write(&asset, other, w(9), w(4), 10),
            write(&asset, asset_balance_key, w(1_000_000), w(1_005_000), 11),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        fields(&events, &oz.vault),
        vec![(pb::StateField::Erc4626TotalAssets as i32, "1000000".into(), "1005000".into(), 1)]
    );
    assert_eq!((&events.global_state[0].key, &events.global_state[0].storage_contract), (&oz.vault, &asset));
    assert!(fields(&events, &sdai.vault).is_empty());
    // Binding: sDAI has no proxy; OZ carries the offset constant.
    let events = project(&block(1), &cfg).unwrap();
    let sdai_roles: Vec<i32> = events.dependencies.iter().filter(|d| d.market == sdai.vault).map(|d| d.role).collect();
    assert_eq!(
        sdai_roles,
        vec![pb::DependencyRole::Underlying as i32, pb::DependencyRole::RateAccumulator as i32]
    );
    assert!(events.epochs.iter().all(|e| e.implementation.is_empty()));
    let offset: Vec<_> = events.global_state.iter().filter(|g| g.market == oz.vault).collect();
    assert_eq!(
        (offset.len(), offset[0].field, &*offset[0].value, offset[0].observation),
        (1, pb::StateField::Erc4626DecimalsOffset as i32, "12", pb::Observation::QualifiedConstant as i32)
    );
    assert_eq!(decimals_offset, 12);
    // Pot code change invalidates sDAI only.
    b.transaction_traces = vec![tx(eth::Call {
        code_changes: vec![eth::CodeChange {
            address: pot.clone(),
            old_hash: vec![1; 32],
            new_hash: vec![2; 32],
            ordinal: 5,
            ..Default::default()
        }],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(
        (events.epochs.len(), &events.epochs[0].market, events.epochs[0].reason),
        (1, &sdai.vault, pb::InvalidationReason::DependencyCodeChange as i32)
    );
}

#[test]
fn reverts_ordering_versions_and_parameters_fail_closed() {
    let cfg = bsc();
    let v = cfg.vaults[0].clone();
    let mut b = block(10);
    let mut call = shares_call(&v, &[9; 20], 1, 2, 10);
    call.state_reverted = true;
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    let mut call = shares_call(&v, &[9; 20], 1, 2, 10);
    let key = mapping_key(&[9; 20], &v.balances_slot);
    call.storage_changes.push(write(&v.vault, key, w(3), w(4), 11));
    b.transaction_traces = vec![tx(call.clone())];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("discontinuous"));
    call.storage_changes[1] = write(&v.vault, key, w(2), w(4), 10);
    b.transaction_traces = vec![tx(call)];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("ambiguous"));
    let mut b = block(10);
    b.ver = 4;
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("producer version"));
    assert!(parse(r#"{"chain_id":56,"producer_versions":[5],"vaults":[]}"#).unwrap().vaults.is_empty());
    let mutate = |src: &str, f: &dyn Fn(&mut serde_json::Value)| {
        let mut v: serde_json::Value = serde_json::from_str(src).unwrap();
        f(&mut v);
        parse(&v.to_string()).map(|_| ()).unwrap_err().to_string()
    };
    assert!(mutate(BSC, &|v| v["vaults"][0]["model"] = "maker-savings-dai".into()).contains("does not match"));
    assert!(mutate(
        BSC,
        &|v| v["vaults"][0]["pot"] =
            serde_json::json!({"address":"0x197E90f9FAD81970bA7976f33CbD77088E5D7cf7","dsr_slot":"0x03","chi_slot":"0x04","rho_slot":"0x07"})
    )
    .contains("exactly one"));
    assert!(mutate(BSC, &|v| v["vaults"][0]["aave"]["implementation"] = serde_json::Value::Null).contains("go together"));
    assert!(mutate(BSC, &|v| v["vaults"][0]["implementation"] = serde_json::Value::Null).contains("go together"));
    assert!(mutate(BSC, &|v| v["vaults"][0]["total_supply_slot"] = v["vaults"][0]["balances_slot"].clone()).contains("overlap"));
    assert!(mutate(MAINNET, &|v| v["vaults"][0]["pot"]["rho_slot"] = v["vaults"][0]["pot"]["chi_slot"].clone()).contains("pot slots overlap"));
    assert!(mutate(MAINNET, &|v| v["vaults"][1]["vault"] = v["vaults"][0]["vault"].clone()).contains("duplicate"));
    assert!(mutate(MAINNET, &|v| v["vaults"][1]["oz"]["fee_bps"] = 1.into()).contains("unknown field"));
    assert!(mutate(BSC, &|v| v["vaults"][0]["source_pin"] = "".into()).contains("source_pin"));
}
