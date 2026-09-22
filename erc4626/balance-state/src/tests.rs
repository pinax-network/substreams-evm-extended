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
            // The unchanged rate half of the written word is still a row (one row per decoded field).
            (
                pb::StateField::AaveCurrentLiquidityRate as i32,
                (3 * ray / 100).to_string(),
                (3 * ray / 100).to_string(),
                1
            ),
            (pb::StateField::AaveLastUpdateTimestamp as i32, "1789689000".into(), "1789689600".into(), 1),
        ]
    );
    let index = &events.global_state[0];
    assert_eq!(
        (&index.key, &index.storage_contract, &*index.scale, index.bit_width),
        (&v.asset, &pool, RAY, 128)
    );
    assert_eq!((events.global_state[1].bit_offset, events.global_state[1].bit_width), (128, 128));
    assert_eq!((events.global_state[2].bit_offset, events.global_state[2].bit_width), (128, 40));
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
        ..
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

/// ERC-7201: `keccak256(abi.encode(uint256(keccak256(id)) - 1)) & ~bytes32(uint256(0xff))`.
fn erc7201(id: &str) -> [u8; 32] {
    let mut h = keccak(id.as_bytes());
    // minus one, big-endian with borrow
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

#[test]
fn the_openzeppelin_namespace_slot_is_derived_from_its_id_and_shared_rules_hold() {
    use prost::Message;
    let cfg = mainnet();
    let oz = cfg.vaults[1].clone();
    assert_eq!(erc7201("openzeppelin.storage.ERC20"), oz.balances_slot);
    assert_eq!(add_offset(&erc7201("openzeppelin.storage.ERC20"), 2), oz.total_supply_slot);
    assert_eq!(add_offset(&erc7201("openzeppelin.storage.ERC20"), 1), oz.other_mapping_slots[0]);
    // Only Extended producer versions 4 and 5 are qualified.
    let mut v: serde_json::Value = serde_json::from_str(BSC).unwrap();
    v["producer_versions"] = serde_json::json!([3]);
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("4 and 5"));
    // A delegatecall frame (Call.address == implementation) writing the vault proxy's storage.
    let cfg = bsc();
    let vlt = cfg.vaults[0].clone();
    let holder = [9u8; 20];
    let key = mapping_key(&holder, &vlt.balances_slot);
    let mut b = block(10);
    b.transaction_traces = vec![eth::TransactionTrace {
        status: eth::TransactionTraceStatus::Succeeded as i32,
        hash: vec![7; 32],
        index: 9,
        calls: vec![
            eth::Call {
                index: 0,
                address: vlt.vault.clone(),
                ..Default::default()
            },
            eth::Call {
                index: 1,
                parent_index: 0,
                depth: 1,
                call_type: eth::CallType::Delegate as i32,
                address: vlt.implementation.clone().unwrap(),
                keccak_preimages: [preimage(&holder, &vlt.balances_slot)].into(),
                storage_changes: vec![write(&vlt.vault, key, w(1), w(2), 10)],
                ..Default::default()
            },
        ],
        ..Default::default()
    }];
    let events = project(&b, &cfg).unwrap();
    assert_eq!((events.holder_basis.len(), &*events.holder_basis[0].value), (1, "2"));
    // FAILED and REVERTED transactions contribute nothing; status 0 is refused.
    for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
        let mut t = tx(shares_call(&vlt, &holder, 1, 2, 10));
        t.status = status as i32;
        b.transaction_traces = vec![t];
        assert!(project(&b, &cfg).unwrap().holder_basis.is_empty());
    }
    let mut t = tx(eth::Call::default());
    t.status = 0;
    b.transaction_traces = vec![t];
    assert!(project(&b, &cfg).unwrap_err().to_string().contains("incomplete transaction"));
    // Blocks before activation emit only the clock.
    let mut v: serde_json::Value = serde_json::from_str(BSC).unwrap();
    v["vaults"][0]["activation_block"] = 100.into();
    let late = parse(&v.to_string()).unwrap();
    let mut b = block(50);
    b.transaction_traces = vec![tx(eth::Call {
        address: vlt.vault.clone(),
        storage_changes: vec![write(&vlt.vault, w(0x77), w(0), w(1), 10)],
        ..Default::default()
    })];
    let events = project(&b, &late).unwrap();
    assert!(events.holder_basis.is_empty() && events.global_state.is_empty() && events.epochs.is_empty());
    assert_eq!(events.clocks.len(), 1);
    // Ordering faults name the contract, key and ordinals.
    let mut call = shares_call(&vlt, &holder, 1, 2, 10);
    call.storage_changes.push(write(&vlt.vault, key, w(3), w(4), 11));
    let mut b = block(10);
    b.transaction_traces = vec![tx(call)];
    let err = project(&b, &cfg).unwrap_err().to_string();
    assert!(err.contains("discontinuous") && err.contains(&hex::encode(&vlt.vault)) && err.contains(&hex::encode(key)) && err.contains("ordinal 11"));
    // Determinism under input permutation.
    let mut b = block(10);
    let mut second = tx(shares_call(&vlt, &[8; 20], 5, 6, 12));
    second.index = 10;
    second.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&vlt, &holder, 1, 2, 10)), second];
    let forward = project(&b, &cfg).unwrap();
    let mut reversed = b.clone();
    reversed.transaction_traces.reverse();
    assert_eq!(forward.encode_to_vec(), project(&reversed, &cfg).unwrap().encode_to_vec());
}

#[test]
fn validate_block_refusals_provenance_and_multi_vault_attribution() {
    let cfg = mainnet();
    let sdai = cfg.vaults[0].clone();
    let oz = cfg.vaults[1].clone();
    type Mutation = Box<dyn Fn(&mut eth::Block)>;
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "Extended blocks required",
            Box::new(|b| b.detail_level = eth::block::DetailLevel::DetaillevelBase as i32),
        ),
        ("producer version", Box::new(|b| b.ver = 3)),
        ("missing header", Box::new(|b| b.header = None)),
        ("invalid block identity", Box::new(|b| b.hash = vec![1; 31])),
        ("invalid block identity", Box::new(|b| b.header.as_mut().unwrap().parent_hash = vec![])),
        ("header number mismatch", Box::new(|b| b.header.as_mut().unwrap().number += 1)),
        ("missing timestamp", Box::new(|b| b.header.as_mut().unwrap().timestamp = None)),
        (
            "negative timestamp",
            Box::new(|b| b.header.as_mut().unwrap().timestamp.as_mut().unwrap().seconds = -1),
        ),
    ];
    for (message, apply) in cases {
        let mut b = block(10);
        apply(&mut b);
        let err = project(&b, &cfg).unwrap_err().to_string();
        assert!(err.contains(message), "expected `{message}`, got `{err}`");
    }
    // Same-block repeated writes keep the first old value and the last write's provenance.
    let holder = [9u8; 20];
    let mut b = block(10);
    let mut second = tx(shares_call(&sdai, &holder, 2, 7, 20));
    second.index = 10;
    second.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&sdai, &holder, 1, 2, 10)), second];
    let events = project(&b, &cfg).unwrap();
    let h = &events.holder_basis[0];
    assert_eq!(
        (
            &*h.previous_value,
            &*h.value,
            h.change_count,
            h.first_ordinal,
            h.ordinal,
            h.transaction_index,
            &h.transaction_hash
        ),
        ("1", "7", 2, 10, 20, 10, &vec![8; 32])
    );
    // Two vaults written in one block are attributed by storage address, with their own decimals.
    let mut b = block(10);
    let mut oz_tx = tx(shares_call(&oz, &holder, 5, 6, 12));
    oz_tx.index = 10;
    oz_tx.hash = vec![8; 32];
    b.transaction_traces = vec![tx(shares_call(&sdai, &holder, 1, 2, 10)), oz_tx];
    let events = project(&b, &cfg).unwrap();
    let rows: Vec<(&Vec<u8>, &str)> = events.holder_basis.iter().map(|h| (&h.market, h.value.as_str())).collect();
    let mut expected = vec![(&sdai.vault, "2"), (&oz.vault, "6")];
    expected.sort();
    assert_eq!(rows, expected);
    let bound = project(&block(1), &cfg).unwrap();
    let decimals: Vec<(Vec<u8>, u32)> = bound.epochs.iter().map(|e| (e.market.clone(), e.balance_decimals)).collect();
    assert!(decimals.contains(&(sdai.vault.clone(), 18)) && decimals.contains(&(oz.vault.clone(), 18)));
}

#[test]
fn oz_asset_decoder_masks_only_the_source_bound_blacklist_bit() {
    let cfg = mainnet();
    let oz = &cfg.vaults[1];
    let Model::OzVirtualOffset { asset_balance_key, .. } = &oz.model else {
        panic!()
    };
    let mut flagged = w(17);
    flagged[0] = 0x80;
    let mut maximum = [0xff; 32];
    maximum[0] = 0x7f;
    for (old, new, previous, value) in [
        (w(17), flagged, "17".to_string(), "17".to_string()),
        (flagged, w(0), "17".into(), "0".into()),
        (w(0), maximum, "0".into(), bits(&maximum, 0, 256).to_string()),
    ] {
        let mut b = block(10);
        b.transaction_traces = vec![tx(eth::Call {
            // A delegatecall writes the proxy's storage.
            address: vec![0x22; 20],
            storage_changes: vec![write(&oz.asset, *asset_balance_key, old, new, 10)],
            ..Default::default()
        })];
        let events = project(&b, &cfg).unwrap();
        assert_eq!(events.global_state.len(), 1);
        let row = &events.global_state[0];
        assert_eq!((&row.previous_value, &row.value), (&previous, &value));
        assert_eq!((row.bit_offset, row.bit_width), (0, 255));
        assert_eq!((&row.raw_previous_word, &row.raw_word), (&old.to_vec(), &new.to_vec()));
        assert_eq!((&row.storage_contract, &row.key), (&oz.asset, &oz.vault));
        assert!(events.holder_basis.is_empty());
    }
    // An explicitly qualified uint256 asset must retain its high balance bit.
    let mut raw: serde_json::Value = serde_json::from_str(MAINNET).unwrap();
    raw["vaults"][1]["oz"]["asset_balance_model"] = "uint256".into();
    raw["vaults"][1]["oz"]["asset_source_pin"] = "synthetic full uint256 mapping fixture".into();
    let cfg = parse(&raw.to_string()).unwrap();
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        storage_changes: vec![write(&oz.asset, *asset_balance_key, w(0), flagged, 10)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.global_state[0].bit_width, 256);
    assert_eq!(events.global_state[0].value, bits(&flagged, 0, 256).to_string());
}

#[test]
fn oz_asset_proxy_binding_invalidates_pointer_and_implementation_changes() {
    let cfg = mainnet();
    let oz = &cfg.vaults[1];
    let Model::OzVirtualOffset {
        asset_implementation_slot: Some(slot),
        asset_implementation: Some(implementation),
        asset_source_pin,
        ..
    } = &oz.model
    else {
        panic!()
    };
    assert_eq!(*slot, keccak(b"org.zeppelinos.proxy.implementation"));
    for number in [1, 1_001] {
        let events = project(&block(number), &cfg).unwrap();
        let rows: Vec<_> = events.dependencies.iter().filter(|d| d.market == oz.vault).collect();
        assert_eq!(rows.len(), 2);
        let dep = rows.iter().find(|d| d.depth == 2).unwrap();
        assert_eq!((&dep.contract, &dep.parent, &dep.pointer_contract), (implementation, &oz.asset, &oz.asset));
        assert_eq!(
            (&dep.pointer_slot, &dep.pointer_value),
            (&slot.to_vec(), &word(implementation).unwrap().to_vec())
        );
        assert_eq!(dep.binding, pb::BindingKind::StoragePointer as i32);
        assert_eq!(dep.role, pb::DependencyRole::Implementation as i32);
        assert!(rows.iter().all(|d| &d.source_pin == asset_source_pin));
        assert_eq!(
            dep.kind,
            if number == 1 {
                pb::EpochEventKind::Bound
            } else {
                pb::EpochEventKind::Reaffirmed
            } as i32
        );
    }
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        address: implementation.clone(),
        storage_changes: vec![
            write(&oz.asset, *slot, w(1), w(2), 10),
            // An upgrade then restoration still invalidates the bound epoch.
            write(&oz.asset, *slot, w(2), w(1), 11),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 2);
    assert_eq!(events.epochs[0].evidence_previous_word, w(1));
    assert_eq!(events.epochs[0].evidence_word, w(2));
    let e = &events.epochs[1];
    assert_eq!(
        (&e.market, e.reason, e.ordinal),
        (&oz.vault, pb::InvalidationReason::DependencyPointerWrite as i32, 11)
    );
    assert_eq!((&e.evidence_contract, &e.evidence_slot), (&oz.asset, &slot.to_vec()));
    assert_eq!((&e.evidence_previous_word, &e.evidence_word), (&w(2).to_vec(), &w(1).to_vec()));
    assert_eq!((&e.transaction_hash, e.transaction_index), (&vec![7; 32], 9));
    b.transaction_traces[0].calls[0].state_reverted = true;
    assert!(project(&b, &cfg).unwrap().epochs.is_empty());
    b.transaction_traces = vec![tx(eth::Call {
        address: oz.asset.clone(),
        storage_changes: vec![write(&oz.asset, *slot, w(1), w(1), 12)],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 1);
    assert_eq!(events.epochs[0].reason, pb::InvalidationReason::DependencyPointerWrite as i32);
    assert_eq!(
        (&events.epochs[0].evidence_previous_word, &events.epochs[0].evidence_word),
        (&w(1).to_vec(), &w(1).to_vec())
    );
    b.transaction_traces[0].calls[0].state_reverted = true;
    assert!(project(&b, &cfg).unwrap().epochs.is_empty());
    // Same-value balance writes do not turn into a fabricated state change.
    let mut b_noop = block(10);
    b_noop.transaction_traces = vec![tx(shares_call(oz, &[4; 20], 5, 5, 13))];
    assert!(project(&b_noop, &cfg).unwrap().holder_basis.is_empty());
    for address in [&oz.asset, implementation] {
        b.transaction_traces = vec![tx(eth::Call {
            code_changes: vec![eth::CodeChange {
                address: address.clone(),
                old_hash: vec![1; 32],
                new_hash: vec![2; 32],
                ordinal: 15,
                ..Default::default()
            }],
            ..Default::default()
        })];
        let events = project(&b, &cfg).unwrap();
        assert_eq!(events.epochs.len(), 1);
        assert_eq!(events.epochs[0].market, oz.vault);
        assert_eq!(events.epochs[0].reason, pb::InvalidationReason::DependencyCodeChange as i32);
        assert_eq!(&events.epochs[0].evidence_contract, address);
        assert_eq!(events.epochs[0].evidence_code_hash, vec![2; 32]);
    }
}

#[test]
fn oz_asset_layout_and_proxy_parameters_are_explicit_and_consistent() {
    let check = |f: &dyn Fn(&mut serde_json::Value), message: &str| {
        let mut raw: serde_json::Value = serde_json::from_str(MAINNET).unwrap();
        f(&mut raw["vaults"][1]);
        let error = parse(&raw.to_string()).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    };
    check(
        &|v| {
            v["oz"].as_object_mut().unwrap().remove("asset_balance_model");
        },
        "asset_balance_model",
    );
    check(&|v| v["oz"]["asset_balance_model"] = "unreviewed-layout".into(), "unknown variant");
    check(&|v| v["oz"]["asset_source_pin"] = " ".into(), "asset_source_pin required");
    check(&|v| v["oz"]["asset_implementation"] = serde_json::Value::Null, "go together");
    check(&|v| v["oz"]["asset_implementation_slot"] = serde_json::Value::Null, "go together");
    check(&|v| v["oz"]["asset_implementation"] = "0x01".into(), "20 bytes");
    check(&|v| v["oz"]["decimals_offset"] = 78.into(), "overflows uint256");
    check(&|v| v["vault_decimals"] = 19.into(), "vault_decimals");
    check(
        &|v| {
            v["vault_decimals"] = 262.into();
            v["asset_decimals"] = 250.into();
        },
        "within uint8",
    );
    check(&|v| v["asset"] = v["vault"].clone(), "differ from vault");
    let Model::OzVirtualOffset { asset_balance_key, .. } = mainnet().vaults[1].model else {
        panic!()
    };
    check(
        &|v| v["oz"]["asset_implementation_slot"] = format!("0x{}", hex::encode(asset_balance_key)).into(),
        "slots overlap",
    );
}

#[test]
fn vault_and_pool_pointer_writes_preserve_noops_restoration_and_persistence() {
    let cfg = bsc();
    let vault = &cfg.vaults[0];
    let Model::AaveStaticAToken {
        pool,
        implementation_slot: Some(pool_slot),
        implementation: Some(pool_implementation),
        ..
    } = &vault.model
    else {
        panic!()
    };
    for (address, slot, implementation, reason) in [
        (
            &vault.vault,
            vault.implementation_slot.unwrap(),
            vault.implementation.as_ref().unwrap(),
            pb::InvalidationReason::ImplementationPointerWrite,
        ),
        (pool, *pool_slot, pool_implementation, pb::InvalidationReason::DependencyPointerWrite),
    ] {
        let expected = word(implementation).unwrap();
        for transitions in [vec![(expected, expected)], vec![(expected, w(2)), (w(2), expected)]] {
            let mut b = block(10);
            b.transaction_traces = transitions
                .iter()
                .enumerate()
                .map(|(i, (old, new))| {
                    let mut t = tx(eth::Call {
                        index: 3 + i as u32,
                        address: implementation.clone(), // delegatecall storage belongs to the proxy
                        storage_changes: vec![write(address, slot, *old, *new, 10 + i as u64)],
                        ..Default::default()
                    });
                    t.index = 9 + i as u32;
                    t.hash = vec![7 + i as u8; 32];
                    t
                })
                .collect();
            let events = project(&b, &cfg).unwrap();
            assert_eq!(events.epochs.len(), transitions.len());
            assert!(events.holder_basis.is_empty() && events.global_state.is_empty());
            for (i, row) in events.epochs.iter().enumerate() {
                assert_eq!((&row.market, row.reason), (&vault.vault, reason as i32));
                assert_eq!(row.kind, pb::EpochEventKind::Invalidated as i32);
                assert_eq!((&row.evidence_contract, &row.evidence_slot), (address, &slot.to_vec()));
                assert_eq!(
                    (&row.evidence_previous_word, &row.evidence_word),
                    (&transitions[i].0.to_vec(), &transitions[i].1.to_vec())
                );
                assert_eq!(
                    (row.ordinal, row.transaction_index, row.call_index),
                    (10 + i as u64, 9 + i as u32, 3 + i as u32)
                );
                assert_eq!(row.transaction_hash, vec![7 + i as u8; 32]);
            }
            for status in [eth::TransactionTraceStatus::Failed, eth::TransactionTraceStatus::Reverted] {
                for t in &mut b.transaction_traces {
                    t.status = status as i32;
                }
                assert!(project(&b, &cfg).unwrap().epochs.is_empty());
            }
            for t in &mut b.transaction_traces {
                t.status = eth::TransactionTraceStatus::Succeeded as i32;
                t.calls[0].state_reverted = true;
            }
            assert!(project(&b, &cfg).unwrap().epochs.is_empty());
            b.system_calls = b
                .transaction_traces
                .iter()
                .map(|t| eth::Call {
                    state_reverted: false,
                    ..t.calls[0].clone()
                })
                .collect();
            b.transaction_traces.clear();
            let events = project(&b, &cfg).unwrap();
            assert_eq!(events.epochs.len(), transitions.len());
            assert!(events
                .epochs
                .iter()
                .all(|e| e.scope == pb::Scope::SystemCall as i32 && e.transaction_hash.is_empty()));
        }
        // True noops participate in ordering/continuity validation like every other pointer write.
        for (second_old, ordinal, error) in [(w(2), 10, "ambiguous"), (w(3), 11, "discontinuous")] {
            let mut b = block(10);
            b.transaction_traces = vec![tx(eth::Call {
                storage_changes: vec![write(address, slot, expected, w(2), 10), write(address, slot, second_old, second_old, ordinal)],
                ..Default::default()
            })];
            let actual = project(&b, &cfg).unwrap_err().to_string();
            assert!(actual.contains(error) && actual.contains(&hex::encode(address)) && actual.contains(&hex::encode(slot)));
        }
    }
    // Ordinary holder, total-supply and reserve-word noops remain suppressed.
    let Model::AaveStaticAToken { reserve_base, .. } = &vault.model else {
        panic!()
    };
    let mut call = shares_call(vault, &[4; 20], 5, 5, 10);
    call.storage_changes.extend([
        write(&vault.vault, vault.total_supply_slot, w(9), w(9), 11),
        write(pool, add_offset(reserve_base, 1), w(12), w(12), 12),
    ]);
    let mut b = block(10);
    b.transaction_traces = vec![tx(call)];
    let events = project(&b, &cfg).unwrap();
    assert!(events.epochs.is_empty() && events.holder_basis.is_empty() && events.global_state.is_empty());
}

#[test]
fn pointer_guards_attribute_shared_pool_and_direct_vault_writes_deterministically() {
    use prost::Message;
    let mut raw: serde_json::Value = serde_json::from_str(BSC).unwrap();
    let mut other = raw["vaults"][0].clone();
    other["vault"] = format!("0x{}", hex::encode([0x33; 20])).into();
    other["epoch"] = 2.into();
    raw["vaults"].as_array_mut().unwrap().push(other);
    let cfg = parse(&raw.to_string()).unwrap();
    let first = &cfg.vaults[0];
    let second = &cfg.vaults[1];
    let Model::AaveStaticAToken {
        pool,
        implementation_slot: Some(slot),
        ..
    } = &first.model
    else {
        panic!()
    };
    let mut b = block(10);
    b.transaction_traces = vec![
        tx(eth::Call {
            storage_changes: vec![write(pool, *slot, w(1), w(1), 10)],
            ..Default::default()
        }),
        tx(eth::Call {
            storage_changes: vec![
                write(&first.vault, first.implementation_slot.unwrap(), w(1), w(2), 20),
                write(&first.vault, first.implementation_slot.unwrap(), w(2), w(1), 30),
            ],
            ..Default::default()
        }),
    ];
    let events = project(&b, &cfg).unwrap();
    assert_eq!(events.epochs.len(), 4);
    assert_eq!(events.epochs.iter().filter(|e| e.market == first.vault && e.epoch == first.epoch).count(), 3);
    let second_rows: Vec<_> = events.epochs.iter().filter(|e| e.market == second.vault).collect();
    assert_eq!(second_rows.len(), 1);
    assert_eq!(
        (second_rows[0].epoch, second_rows[0].reason),
        (2, pb::InvalidationReason::DependencyPointerWrite as i32)
    );
    b.transaction_traces.reverse();
    for t in &mut b.transaction_traces {
        t.calls[0].storage_changes.reverse();
    }
    assert_eq!(events.encode_to_vec(), project(&b, &cfg).unwrap().encode_to_vec());
}

#[test]
fn an_openzeppelin_initializing_block_is_reviewed_and_rebinding_the_asset_invalidates() {
    let cfg = mainnet();
    let oz = cfg.vaults[1].clone();
    let Model::OzVirtualOffset { erc4626_storage_slot, .. } = oz.model.clone() else {
        panic!()
    };
    let erc4626_slot = erc4626_storage_slot.expect("the fixture binds the ERC4626Storage namespace slot");
    // `__ERC20_init_unchained` writes `_name` (+3) and `_symbol` (+4) of the
    // openzeppelin.storage.ERC20 namespace; `__ERC4626_init_unchained` writes
    // `_asset` and `_underlyingDecimals`, which share the ERC4626 namespace word.
    let name_slot = add_offset(&oz.balances_slot, 3);
    let symbol_slot = add_offset(&oz.balances_slot, 4);
    assert!(oz.other_slots.contains(&name_slot) && oz.other_slots.contains(&symbol_slot));
    let mut asset_word = [0u8; 32];
    asset_word[12..].copy_from_slice(&oz.asset);
    asset_word[11] = 6; // _underlyingDecimals packed above the address
    let mut b = block(10);
    b.transaction_traces = vec![tx(eth::Call {
        address: oz.vault.clone(),
        storage_changes: vec![
            write(&oz.vault, name_slot, w(0), w(0x6161), 10),
            write(&oz.vault, symbol_slot, w(0), w(0x6262), 11),
            write(&oz.vault, oz.total_supply_slot, w(0), w(1_000), 12),
            write(&oz.vault, erc4626_slot, [0; 32], asset_word, 13),
        ],
        ..Default::default()
    })];
    let events = project(&b, &cfg).unwrap();
    // The metadata writes are reviewed, the supply is carried, and rebinding
    // the asset invalidates the epoch with evidence instead of failing.
    assert_eq!(
        fields(&events, &oz.vault),
        vec![(pb::StateField::Erc4626TotalSupply as i32, "0".into(), "1000".into(), 1)]
    );
    let invalidations: Vec<_> = events.epochs.iter().filter(|e| e.kind == pb::EpochEventKind::Invalidated as i32).collect();
    assert_eq!(invalidations.len(), 1);
    assert_eq!(
        (invalidations[0].reason, &invalidations[0].evidence_slot, &invalidations[0].evidence_word),
        (
            pb::InvalidationReason::DependencyPointerWrite as i32,
            &erc4626_slot.to_vec(),
            &asset_word.to_vec()
        )
    );
    // Without the binding the same block fails closed, which is the defect
    // this test pins: every initializing or reinitializing block was refused.
    let mut v: serde_json::Value = serde_json::from_str(MAINNET).unwrap();
    v["vaults"][1]["oz"].as_object_mut().unwrap().remove("erc4626_storage_slot");
    v["vaults"][1]["other_slots"] = serde_json::json!([]);
    let unbound = parse(&v.to_string()).unwrap();
    assert!(project(&b, &unbound).unwrap_err().to_string().contains("unresolved"));
    // The bound slot may not collide with a decoded one.
    let mut v: serde_json::Value = serde_json::from_str(MAINNET).unwrap();
    v["vaults"][1]["oz"]["erc4626_storage_slot"] = v["vaults"][1]["total_supply_slot"].clone();
    assert!(parse(&v.to_string()).unwrap_err().to_string().contains("overlap"));
}
