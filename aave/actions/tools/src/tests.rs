use super::*;

const TX: &str = "0xdaaaaba63e467a4e93acac5a77eca200fb859102a017dd92e3aeb3301faa3ce8";
const HASH: &str = "0x5374379c80a8e0bda920a6ce3bb097d6bfe2915148b760b6cd56799bb1ed9e8f";
const PARENT: &str = "0xa0199966eee0de097f3d46135d2d09f9a3a394f6fc76dcf8a4ad2442d3ff05bc";
const POOL: &str = "0x6807dc923806fe8fd134338eabca509979a7e0cb";
const USDT: &str = "0x55d398326f99059ff775485246999027b3197955";
const USER: &str = "0x158d2e4e90647d31ae2dd16a011d87d435f1e879";

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}
fn pad(address: &str) -> String {
    format!("0x{:0>64}", address.trim_start_matches("0x"))
}
/// The captured live row of block 123535472 (a USDT `Supply`).
fn line(amount: &str) -> String {
    json!({"@module": "map_events", "@block": 123535472, "@data": {
        "clocks": [{"number": "123535472", "hash": HASH, "parentHash": PARENT}],
        "actions": [
            {"transactionHash": TX, "logIndex": 12, "blockIndex": 320, "pool": POOL, "kind": "ACTION_KIND_SUPPLY",
             "reserve": USDT, "actor": USER, "beneficiary": USER, "amount": amount, "persisted": true},
            // An attempt in a reverted frame: counted, never matched to a receipt.
            {"transactionHash": TX, "logIndex": 5, "blockIndex": 313, "pool": POOL, "kind": "ACTION_KIND_WITHDRAW",
             "reserve": USDT, "actor": USER, "beneficiary": USER, "recipient": USER, "amount": "1"}
        ]
    }})
    .to_string()
}
struct Fake {
    logs: Vec<Value>,
}
impl Chain for Fake {
    fn call(&self, method: &str, _params: Value) -> Result<Value> {
        Ok(match method {
            "eth_getLogs" => json!(self.logs),
            "eth_getBlockByNumber" => json!({"hash": HASH, "parentHash": PARENT}),
            "eth_getStorageAt" => json!(pad("0x5e2b0fcc5b9734c7ec0a03401ee9e6805f783b6d")),
            "eth_getCode" => json!("0x6000"),
            other => bail!("unexpected {other}"),
        })
    }
}
fn supply_log() -> Value {
    json!({
        "blockNumber": "0x75d0070", "blockHash": HASH, "transactionHash": TX, "logIndex": "0x140", "removed": false,
        "topics": [format!("0x{}", aave_actions::SUPPLY_TOPIC), pad(USDT), pad(USER), pad("0x0")],
        "data": format!("0x{}{:064x}", &pad(USER)[2..], 120_000_000_000_000_000_000u128),
    })
}
fn cli(dir: &std::path::Path, output: &str, amount: &str) -> Cli {
    let events = dir.join(format!("{output}.jsonl"));
    std::fs::write(&events, line(amount)).unwrap();
    let spkg = dir.join("p.spkg");
    std::fs::write(&spkg, b"spkg").unwrap();
    Cli {
        events: vec![events],
        params: root().join("tests/fixtures/bsc-aave-v3-pool.json"),
        spkg,
        endpoint: "bsc.substreams.example:443".into(),
        log_span: 1000,
        output: dir.join(output),
    }
}

#[test]
fn a_persisted_row_equal_to_its_receipt_log_passes_and_attempts_are_counted() {
    let tmp = tempfile::tempdir().unwrap();
    let args = cli(tmp.path(), "ok", "120000000000000000000");
    assert!(check(&args, &Fake { logs: vec![supply_log()] }).unwrap());
    let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["checks"]["receipt_logs"]["matched"], 1);
    assert_eq!(
        (report["rows"]["persisted"].as_u64(), report["rows"]["attempted_in_reverted_frames"].as_u64()),
        (Some(1), Some(1))
    );
    assert!(report["endpoints"]["rpc"].as_str().unwrap().contains("not recorded"));
}

#[test]
fn a_field_difference_a_missing_row_and_an_extra_row_each_fail() {
    let tmp = tempfile::tempdir().unwrap();
    // The amount differs from the receipt log.
    let args = cli(tmp.path(), "amount", "1");
    assert!(!check(&args, &Fake { logs: vec![supply_log()] }).unwrap());
    let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["checks"]["receipt_logs"]["field_mismatches"], 1);
    // A receipt log the package did not emit.
    let mut other = supply_log();
    other["logIndex"] = json!("0x141");
    let args = cli(tmp.path(), "missing", "120000000000000000000");
    assert!(!check(
        &args,
        &Fake {
            logs: vec![supply_log(), other]
        }
    )
    .unwrap());
    // A persisted row without a receipt log.
    let args = cli(tmp.path(), "extra", "120000000000000000000");
    assert!(!check(&args, &Fake { logs: vec![] }).unwrap());
    let report: Value = serde_json::from_str(&std::fs::read_to_string(args.output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["checks"]["receipt_logs"]["row_without_rpc_log"], 1);
}

#[test]
fn the_abi_driven_decoder_names_every_bound_event_and_refuses_bad_words() {
    let names: Vec<String> = events().unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["Supply", "Withdraw", "Borrow", "Repay", "LiquidationCall", "FlashLoan"]);
    assert_eq!(screaming("LiquidationCall"), "LIQUIDATION_CALL");
    assert_eq!(
        decimal(&[0xff; 32]),
        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
    );
    assert!(value("bool", &[2u8; 32]).is_err());
    let mut unpadded = [0u8; 32];
    unpadded[0] = 1;
    assert!(value("address", &unpadded).is_err());
}
