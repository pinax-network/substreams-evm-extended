//! Synthetic Extended frames from measured SSTORE/preimage witnesses.
use super::{
    cases::{balance, holders, Measured},
    vm, Target,
};
use crate::ptoken_proof::cases::get;
use anyhow::{ensure, Context, Result};
use primitive_types::U256;
use prost::Message;
use serde_json::{json, Value};
use substreams_ethereum::pb::eth::v2 as eth;
pub fn layout(t: Target) -> Result<Value> {
    let raw = include_bytes!("../../../tests/fixtures/typed450-offline/cases.json");
    ensure!(
        super::sha(raw) == "b527f69a5e118b3b05fcc8c2bd7ec37fdaee65ca656d81dd7a7c8bbd415c108b",
        "historical layouts pin"
    );
    let v: Value = serde_json::from_slice(raw)?;
    let mut l = v
        .as_array()
        .context("cases")?
        .iter()
        .find(|c| c["contract"] == t.address())
        .context("target layout")?["layout"]
        .clone();
    if t == Target::Dsg {
        l["enumerable_address_sets"] = json!([{"root":format!("0x{:064x}",8),"key_types":["bytes32"],"semantics":"oz_3_4_2"}]);
    }
    Ok(l)
}
pub fn project(t: Target, b: &eth::Block) -> Result<proto::pb::evm::balances::v1::Events> {
    let l = erc20_balances::layout::parse(&json!([layout(t)?]).to_string())?;
    erc20_balances::project(b, &l)
}
pub fn captured(t: Target) -> Result<eth::Block> {
    let raw: &[u8] = if t == Target::Apd {
        include_bytes!("../../../tests/fixtures/typed450-offline/rank401-122288154.pb")
    } else {
        include_bytes!("../../../tests/fixtures/typed450-offline/rank418-122288046.pb")
    };
    let digest = if t == Target::Apd {
        "4a637c8760aa18d789863cc49fd203ecd94c04a8a55a483a4b10a049c54a660f"
    } else {
        "9d9ba13340e958cd40d4ee011971a21744eda6d9f5f443fd31582821e36583ea"
    };
    ensure!(super::sha(raw) == digest, "unchanged captured full block");
    Ok(eth::Block::decode(raw)?)
}
pub fn synthetic(t: Target, c: eth::Call) -> Result<eth::Block> {
    let original = captured(t)?;
    Ok(eth::Block {
        ver: 5,
        number: original.number,
        hash: vec![0xa5; 32],
        header: original.header,
        detail_level: eth::block::DetailLevel::DetaillevelExtended as i32,
        transaction_traces: vec![eth::TransactionTrace {
            status: eth::TransactionTraceStatus::Succeeded as i32,
            begin_ordinal: 1,
            end_ordinal: 100_000,
            calls: vec![c],
            ..Default::default()
        }],
        ..Default::default()
    })
}
pub fn row(t: Target, key: U256, old: U256, new: U256, ordinal: u64) -> eth::StorageChange {
    eth::StorageChange {
        address: hex::decode(&t.address()[2..]).unwrap(),
        key: vm::word(key).to_vec(),
        old_value: vm::word(old).to_vec(),
        new_value: vm::word(new).to_vec(),
        ordinal,
    }
}
pub fn measured(m: &Measured, drop_equal: bool) -> Result<eth::Block> {
    let mut c = eth::Call {
        address: hex::decode(&m.target.address()[2..])?,
        index: 0,
        begin_ordinal: 1,
        end_ordinal: 99_999,
        state_reverted: !matches!(m.execution.exit, vm::Exit::Return(_)),
        ..Default::default()
    };
    for h in &m.execution.keccaks {
        c.keccak_preimages.insert(hex::encode(vm::word(h.output)), hex::encode(&h.input));
    }
    for w in &m.execution.writes {
        if !drop_equal || w.old != w.new {
            c.storage_changes.push(row(m.target, w.key, w.old, w.new, (w.step + 10) as u64));
        }
    }
    synthetic(m.target, c)
}
pub fn verify_operation(m: &Measured, getter: Option<&vm::Execution>) -> Result<Value> {
    let mut combinations = 0;
    for drop_equal in [false, true] {
        let b = measured(m, drop_equal)?;
        ensure!(project(m.target, &b)?.balances.is_empty(), "{} metadata-only output", m.name);
        if !matches!(m.execution.exit, vm::Exit::Return(_)) {
            continue;
        }
        let who = holders(m.target)[0];
        let old = get(&m.execution.committed, balance(who));
        let new = U256::from(987);
        let mut mixed = b;
        let c = &mut mixed.transaction_traces[0].calls[0];
        let preimage = [vm::word(who), vm::word(0.into())].concat();
        c.keccak_preimages.insert(hex::encode(vm::word(balance(who))), hex::encode(preimage));
        c.storage_changes.push(row(m.target, balance(who), old, new, 90_000));
        let getter = getter.context("recorded independent mixed getter")?;
        ensure!(
            getter.exit == vm::Exit::Return(vm::word(new).to_vec())
                && getter.writes.is_empty()
                && getter.reads.iter().all(|r| r.key == balance(who))
                && !getter.reads.is_empty(),
            "independent mixed getter"
        );
        let rows = project(m.target, &mixed)?.balances;
        ensure!(
            rows.len() == 1
                && rows[0].contract == Some(hex::decode(&m.target.address()[2..])?)
                && rows[0].address == vm::word(who)[12..]
                && rows[0].amount == new.to_string(),
            "mixed projection agrees with getter"
        );
        combinations += 1;
    }
    Ok(
        json!({"name":m.name,"target":m.target.label(),"metadata_only":true,"mixed_balance_getter_combinations":combinations,"equal_sstore_modes":["complete","equal writes omitted"],"synthetic":true,"qualified":false}),
    )
}

pub fn verify_raw_metadata(t: Target, name: &str, before: &vm::State, after: &vm::State, keys: &[U256], getter: &vm::Execution) -> Result<Value> {
    let mut c = eth::Call {
        address: hex::decode(&t.address()[2..])?,
        begin_ordinal: 1,
        end_ordinal: 99_999,
        ..Default::default()
    };
    for data in [
        [vm::word(44.into()), vm::word(1.into())].concat(),
        [vm::word(55.into()), vm::word(crate::ptoken_proof::cases::mapping(44.into(), 1.into()))].concat(),
        [vm::word(44.into()), vm::word(t.nonce_root())].concat(),
    ] {
        c.keccak_preimages.insert(hex::encode(vm::word(vm::hash(&data))), hex::encode(data));
    }
    for (i, key) in keys.iter().enumerate() {
        c.storage_changes.push(row(t, *key, get(before, *key), get(after, *key), 10 + i as u64));
    }
    let b = synthetic(t, c)?;
    ensure!(project(t, &b)?.balances.is_empty(), "raw metadata-only output");
    let who = holders(t)[0];
    let mut b = b;
    let c = &mut b.transaction_traces[0].calls[0];
    let data = [vm::word(who), vm::word(0.into())].concat();
    c.keccak_preimages.insert(hex::encode(vm::word(balance(who))), hex::encode(data));
    c.storage_changes.push(row(t, balance(who), get(after, balance(who)), 987.into(), 90_000));
    ensure!(
        getter.exit == vm::Exit::Return(vm::word(987.into()).to_vec())
            && getter.reads.iter().all(|r| r.key == balance(who))
            && !getter.reads.is_empty()
            && getter.writes.is_empty(),
        "recorded raw metadata mixed getter"
    );
    let rows = project(t, &b)?.balances;
    ensure!(
        rows.len() == 1 && rows[0].amount == "987" && rows[0].address == vm::word(who)[12..] && rows[0].contract == Some(hex::decode(&t.address()[2..])?),
        "mixed raw metadata projection"
    );
    Ok(
        json!({"target":t.label(),"name":name,"kind":"synthetic raw metadata writes; not setter reachability","metadata_only":true,"mixed_balance_getter_combinations":1,"writes":keys.len(),"qualified":false}),
    )
}
