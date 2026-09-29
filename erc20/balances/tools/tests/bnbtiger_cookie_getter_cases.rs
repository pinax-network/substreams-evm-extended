#![cfg(not(target_arch = "wasm32"))]
use erc20_balances_tools::{
    bnbtiger_cookie_proof as p,
    ptoken_proof::cases::{call, mapping},
};
use primitive_types::U256;
use serde_json::Value;
use std::{fs, path::PathBuf, sync::OnceLock};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/bnbtiger-cookie-getter-proof")
}
fn all() -> &'static Vec<Value> {
    static ALL: OnceLock<Vec<Value>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut records = vec![];
        for t in p::Target::ALL {
            let c = p::verify_capture(t.captured(), t).unwrap();
            let raw = fs::read(fixture().join(format!("{}-compiler-output.json", t.label()))).unwrap();
            let o = p::verify_compiled(&c, &raw, t).unwrap();
            let counts = p::cases::run(&c, &o, t, &mut |r| {
                records.push(r.clone());
                Ok(())
            })
            .unwrap();
            assert_eq!(counts.calls, counts.pairs * 2);
            assert_eq!(counts.calls, counts.returns + counts.reverts);
        }
        records
    })
}
#[test]
fn paired_compiler_and_capture_getters_match_full_width_balances() {
    let all = all();
    let base = all.iter().filter(|c| c["category"] == "balance_domain").collect::<Vec<_>>();
    assert_eq!(base.len(), 128);
    for r in base {
        assert_eq!(r["execution"]["exit"]["kind"], "return");
        assert_eq!(r["execution"]["reads"].as_array().unwrap().len(), 1);
        assert!(r["execution"]["writes"].as_array().unwrap().is_empty());
        assert!(r["execution"]["logs"].as_array().unwrap().is_empty());
    }
}
#[test]
fn malformed_abi_and_trailing_data_are_compiler_specific() {
    for target in ["bnbtiger", "cookie"] {
        let all = all().iter().filter(|r| r["target"] == target).collect::<Vec<_>>();
        let dirty = all.iter().filter(|r| r["case"] == "dirty_address_high_bits").collect::<Vec<_>>();
        assert_eq!(dirty.len(), 4);
        for r in dirty {
            assert_eq!(r["execution"]["exit"]["kind"], if target == "bnbtiger" { "revert" } else { "return" });
        }
        for r in all {
            if r["case"] == "trailing_calldata" {
                assert_eq!(r["execution"]["exit"]["kind"], "return");
            }
            if r["case"] == "calldata_prefix_0" {
                assert_eq!(r["execution"]["exit"]["data"], "");
                assert_eq!(r["execution"]["exit"]["kind"], "return");
            }
        }
    }
}
#[test]
fn cookie_checkpoint_paths_have_uint32_key_and_two_distinct_words() {
    let t = p::Target::Cookie;
    let c = p::verify_capture(t.captured(), t).unwrap();
    let cells = p::cases::metadata_cells(&c, t).unwrap();
    let cp = cells.iter().filter(|c| c.label.starts_with("checkpoints.")).collect::<Vec<_>>();
    assert_eq!(cp.len(), 2);
    let key = mapping(1.into(), mapping(44.into(), 15.into()));
    assert_eq!(cp[0].key, key);
    assert_eq!(cp[0].bytes, 4);
    assert_eq!(cp[1].key, key + 1);
    assert_eq!(cp[1].bytes, 32);
    let packed = cells.iter().filter(|c| c.key == U256::from(6)).collect::<Vec<_>>();
    assert_eq!(
        packed.iter().map(|c| (c.offset, c.bytes)).collect::<Vec<_>>(),
        vec![(0, 1), (1, 2), (3, 2), (5, 2)]
    );
    assert!(cells.iter().any(|c| c.label == "_name.hypothetical_long_data"));
}
#[test]
fn each_metadata_field_and_packed_member_is_an_independent_probe() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let cells = p::cases::metadata_cells(&c, t).unwrap();
        let records = all()
            .iter()
            .filter(|r| r["target"] == t.label() && r["category"] == "metadata_independence")
            .collect::<Vec<_>>();
        assert_eq!(records.len(), cells.len() * 4 * 2 * 2);
        for cell in cells {
            assert_eq!(
                records
                    .iter()
                    .filter(|r| r["case"].as_str().unwrap().starts_with(&format!("metadata_{}_v", cell.label)))
                    .count(),
                16
            );
        }
        for r in records {
            assert_eq!(r["execution"]["reads"].as_array().unwrap().len(), 1);
            assert_eq!(r["expected"]["value"], format!("0x{:064x}", 123));
        }
    }
}
#[test]
fn source_attribution_of_getter_sload_is_exact() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let raw = fs::read(fixture().join(format!("{}-compiler-output.json", t.label()))).unwrap();
        let o = p::verify_compiled(&c, &raw, t).unwrap();
        let code = p::bytes(&p::selected(&o, t)["evm"]["deployedBytecode"]["object"]).unwrap();
        let map = p::source_map::decode(&code, p::selected(&o, t)["evm"]["deployedBytecode"]["sourceMap"].as_str().unwrap()).unwrap();
        let sources = p::mapped_sources(&c, &o, t, "deployedBytecode").unwrap();
        let mut pcs = std::collections::BTreeSet::new();
        for r in all().iter().filter(|r| r["target"] == t.label()) {
            for read in r["execution"]["reads"].as_array().unwrap() {
                pcs.insert(read["pc"].as_u64().unwrap() as usize);
            }
        }
        assert_eq!(pcs.len(), 1);
        for pc in pcs {
            let source = p::source_map::describe(pc, &map, &sources).unwrap();
            assert_eq!(
                source["path"],
                if t == p::Target::Bnbtiger {
                    "BNBTiger.sol"
                } else {
                    "/C/Users/wesle/Desktop/Deploying Tokens/goose-contracts-referral/contracts/libs/BEP20.sol"
                }
            );
            assert!(source["text"].as_str().unwrap().contains("_balances[account]"));
        }
    }
}
#[test]
fn false_getter_success_cannot_hide_wrong_reads_or_effects() {
    for t in p::Target::ALL {
        let c = p::verify_capture(t.captured(), t).unwrap();
        let key = mapping(44.into(), t.root());
        let pre = p::vm::State::from([(key, 123.into())]);
        let e = p::vm::execute(&p::runtime(&c).unwrap(), &call("balanceOf(address)", &[44.into()]), 1.into(), t.account(), &pre);
        p::cases::verify_getter(&e, t, 44.into(), 123.into(), &pre).unwrap();
        let mut wrong = e.clone();
        wrong.reads[0].key = U256::zero();
        assert!(p::cases::verify_getter(&wrong, t, 44.into(), 123.into(), &pre).is_err());
        let mut extra = e.clone();
        extra.reads.push(extra.reads[0].clone());
        assert!(p::cases::verify_getter(&extra, t, 44.into(), 123.into(), &pre).is_err());
        let mut invalid = e.clone();
        invalid.exit = p::vm::Exit::Invalid;
        assert!(p::cases::verify_getter(&invalid, t, 44.into(), 123.into(), &pre).is_err());
        let mut unsupported = e;
        unsupported.exit = p::vm::Exit::HarnessFailure("unsupported".into());
        assert!(p::cases::verify_getter(&unsupported, t, 44.into(), 123.into(), &pre).is_err());
    }
}
