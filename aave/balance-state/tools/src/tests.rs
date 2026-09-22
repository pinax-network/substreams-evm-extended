use crate::cli::Cli;
use crate::replay::{run, Replay};
use clap::Parser;
use serde_json::Value;
use std::{fs, path::Path};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn captured_fixtures_replay_with_index_oracle_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("blocks");
    fs::create_dir(&dir).unwrap();
    let fixtures = root().join("aave/balance-state/tests/fixtures");
    for (file, height) in [
        ("122288220-tx71-ausdt-withdraw.pb", 122288220u64),
        ("122288734-tx81-ausdt-supply.pb", 122288734),
        ("122288932-tx1-ausdt-ausdc-two-reserves.pb", 122288932),
    ] {
        fs::copy(fixtures.join(file), dir.join(format!("{height}.pb"))).unwrap();
    }
    let ok = run(Replay {
        blocks: vec![dir],
        params: fixtures.join("bsc-aave-v3-epochs.json"),
        output: tmp.path().join("out"),
    })
    .unwrap();
    let report: Value = serde_json::from_str(&fs::read_to_string(tmp.path().join("out/report.json")).unwrap()).unwrap();
    assert!(ok, "{report}");
    assert_eq!(report["status"], "replayed");
    assert_eq!(report["replayed_blocks"], 3);
    assert_eq!(report["holder_basis_rows"], 4);
    assert_eq!(report["global_state_rows"], 16);
    assert_eq!(report["index_oracle"]["checks"], 4);
    assert!(report["index_oracle"]["mismatches"].as_array().unwrap().is_empty());
    // Non-adjacent blocks: continuity is skipped, not asserted.
    assert_eq!(report["continuity"]["global_checks"], 0);
    assert!(report["continuity"]["global_skipped_across_gaps"].as_u64().unwrap() > 0);
}

#[test]
fn cli_requires_blocks_params_and_output() {
    assert!(Cli::try_parse_from(["tools", "replay", "--blocks", "x", "--params", "p.json", "--output", "o"]).is_ok());
    assert!(Cli::try_parse_from(["tools", "replay", "--blocks", "x", "--output", "o"]).is_err());
}

mod live_parity {
    use crate::live::{check, selector, Chain, LiveParity};
    use anyhow::{bail, Result};
    use num_bigint::BigUint;
    use serde_json::{json, Value};
    use std::fs;

    const POOL: &str = "0x6807dc923806fe8fd134338eabca509979a7e0cb";
    const ATOKEN: &str = "0xa9251ca9de909cb71783723713b21e4233fbf1b1";
    const USDT: &str = "0x55d398326f99059ff775485246999027b3197955";
    const HOLDER: &str = "0xe863f97da3601781f366711f5bbfe982f0c450da";
    const HASH: &str = "0x56d563e17b3fa5e05063634fca049fe29df3ee055f2e5ae0d53bcec33b79937a";
    const PARENT: &str = "0xf54694ea00b361b5a60652a9b2ca8623b4c9c79459b3615aa4a46ae6570e9d2b";
    const ROOT: &str = "0xbe667c4ae4a75cdf735f1c0c0c2a77285d18523674b68870840e4486f956fe5e";
    const TIMESTAMP: u64 = 1_790_111_199;
    const INDEX: &str = "1125033357181668499046299166";
    const RATE: &str = "29191869605953944653026341";
    const SCALED: &str = "45231569380410354393";

    fn word(v: &str) -> String {
        format!("{:0>64}", v.parse::<BigUint>().unwrap().to_str_radix(16))
    }
    /// A packaged-output line shaped like `substreams run -o jsonl --bytes-encoding hex`.
    fn event_line() -> String {
        let global = |field: &str, key: &str, value: &str| json!({"market": ATOKEN, "field": field, "key": key, "value": value});
        json!({"@module": "map_events", "@block": 123442210, "@data": {
            "clocks": [{"number": "123442210", "hash": HASH, "parentHash": PARENT, "stateRoot": ROOT, "timestamp": TIMESTAMP.to_string(),
                        "package": "aave_balance_state", "packageVersion": "0.1.0", "parametersSha256": "02"}],
            "holderBasis": [{"market": ATOKEN, "holder": HOLDER, "value": SCALED}],
            "globalState": [
                global("STATE_FIELD_AAVE_LIQUIDITY_INDEX", USDT, INDEX),
                global("STATE_FIELD_AAVE_CURRENT_LIQUIDITY_RATE", USDT, RATE),
                global("STATE_FIELD_AAVE_LAST_UPDATE_TIMESTAMP", USDT, &TIMESTAMP.to_string()),
                global("STATE_FIELD_AAVE_SCALED_TOTAL_SUPPLY", ATOKEN, "999"),
            ]
        }})
        .to_string()
    }
    /// Answers every getter from the values above; `scaled_balance` can be
    /// tampered with to prove that a mismatch is reported, not tolerated.
    struct Fake {
        scaled_balance: String,
    }
    impl Chain for Fake {
        fn call(&self, method: &str, params: Value) -> Result<Value> {
            Ok(match method {
                "eth_getBlockByNumber" => json!({"hash": HASH, "parentHash": PARENT, "stateRoot": ROOT, "timestamp": format!("{TIMESTAMP:#x}")}),
                "eth_getStorageAt" => {
                    let implementation = if params[0].as_str() == Some(POOL) {
                        "5e2b0fcc5b9734c7ec0a03401ee9e6805f783b6d"
                    } else {
                        "7e199fc666368d95b9eaefa7d2d8081acab74134"
                    };
                    json!(format!("0x{implementation:0>64}"))
                }
                "eth_getCode" => json!("0x6000"),
                "eth_call" => {
                    let data = params[0]["data"].as_str().unwrap().trim_start_matches("0x").to_string();
                    let words: Vec<String> = if data.starts_with(&selector("getReserveData(address)")) {
                        let mut w = vec![word("0"); 15];
                        w[1] = word(INDEX);
                        w[2] = word(RATE);
                        w[6] = word(&TIMESTAMP.to_string());
                        w
                    } else if data.starts_with(&selector("scaledBalanceOf(address)")) {
                        vec![word(&self.scaled_balance)]
                    } else if data.starts_with("70a08231") {
                        // Same-second update: the stored index, floor rounding.
                        let scaled: BigUint = SCALED.parse().unwrap();
                        let index: BigUint = INDEX.parse().unwrap();
                        vec![word(&(scaled * index / BigUint::from(10u8).pow(27)).to_string())]
                    } else if data.starts_with(&selector("scaledTotalSupply()")) {
                        vec![word("999")]
                    } else if data.starts_with(&selector("ATOKEN_REVISION()")) {
                        vec![word("5")]
                    } else {
                        bail!("unexpected eth_call {data}")
                    };
                    json!(format!("0x{}", words.concat()))
                }
                other => bail!("unexpected method {other}"),
            })
        }
        fn calls(&self) -> u64 {
            0
        }
    }
    fn args(dir: &std::path::Path, output: &str) -> LiveParity {
        let events = dir.join("events.jsonl");
        fs::write(&events, event_line() + "\n").unwrap();
        let spkg = dir.join("package.spkg");
        fs::write(&spkg, b"spkg").unwrap();
        LiveParity {
            events: vec![events],
            params: super::root().join("aave/balance-state/tests/fixtures/bsc-aave-v3-epochs.json"),
            spkg,
            endpoint: "bsc.substreams.example:443".into(),
            output: dir.join(output),
        }
    }

    #[test]
    fn matching_getters_pass_and_the_report_binds_package_events_and_identities() {
        let tmp = tempfile::tempdir().unwrap();
        let a = args(tmp.path(), "ok");
        assert!(check(&a, &Fake { scaled_balance: SCALED.into() }).unwrap());
        let report: Value = serde_json::from_str(&fs::read_to_string(a.output.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["status"], "passed");
        for (check, count) in [
            ("scaled_balance", 1),
            ("balance_of", 1),
            ("reserve_words", 3),
            ("scaled_total_supply", 1),
            ("clock", 4),
        ] {
            assert_eq!(
                (report["checks"][check]["checked"].as_u64(), report["checks"][check]["mismatches"].as_u64()),
                (Some(count), Some(0)),
                "{check}"
            );
        }
        assert_eq!(report["events_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(report["atoken_revisions"][0]["rpc"], "5");
        assert!(report["endpoints"]["rpc"].as_str().unwrap().contains("not recorded"));
        // A second run into the same directory is refused.
        assert!(check(&a, &Fake { scaled_balance: SCALED.into() }).is_err());
    }

    #[test]
    fn a_scaled_balance_that_differs_from_the_getter_fails_the_report() {
        let tmp = tempfile::tempdir().unwrap();
        let a = args(tmp.path(), "bad");
        assert!(!check(&a, &Fake { scaled_balance: "1".into() }).unwrap());
        let report: Value = serde_json::from_str(&fs::read_to_string(a.output.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["status"], "failed");
        assert_eq!(report["checks"]["scaled_balance"]["mismatches"], 1);
        assert_eq!(report["checks"]["scaled_balance"]["mismatch_examples"][0]["rpc"], "1");
    }
}
