use super::*;
use crate::{
    capture,
    cli::{record_run, Cli, Commands},
    rpc::ensure_finalized,
};
use clap::Parser;
use std::{fs, path::Path, sync::Mutex};

fn configured(name: &str, chain_id: u64) -> VerificationNetwork {
    VerificationNetwork {
        network: Some(name.into()),
        expected_chain_id: Some(chain_id),
    }
}

struct ChainRpc {
    chain: Value,
    calls: Mutex<Vec<String>>,
}
impl ChainRpc {
    fn new(chain: Value) -> Self {
        Self {
            chain,
            calls: Mutex::new(Vec::new()),
        }
    }
}
impl Rpc for ChainRpc {
    fn request(&self, payload: Value) -> Result<Value> {
        let method = payload["method"].as_str().unwrap();
        self.calls.lock().unwrap().push(method.into());
        let result = match method {
            "eth_chainId" => self.chain.clone(),
            "eth_getBlockByNumber" => {
                assert_eq!(payload["params"], json!(["finalized", false]));
                json!({"number":"0x100"})
            }
            _ => panic!("unexpected data request before network verification: {method}"),
        };
        Ok(json!({"jsonrpc":"2.0","id":payload["id"],"result":result}))
    }
}

#[test]
fn wrong_chain_stops_before_finality_and_reports_requested_and_actual_identity() {
    let rpc = ChainRpc::new(json!("0x38"));
    let network = configured("base", 8453).selected().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("mismatch");
    let mut proceeded = false;
    assert!(!record_run(&output, json!({"status":"incomplete"}), |report| {
        network.verify_rpc(&rpc, report)?;
        proceeded = true;
        ensure_finalized(&rpc, 42)
    })
    .unwrap());
    assert!(!proceeded);
    assert_eq!(*rpc.calls.lock().unwrap(), ["eth_chainId"]);
    let report: Value = serde_json::from_slice(&fs::read(output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["network"], "base");
    assert_eq!(report["expected_chain_id"], 8453);
    assert_eq!(report["rpc_chain_id"], 56);
    assert!(report.get("chain_id").is_none());
    assert_eq!(report["network_binding"], "configured_only");
    assert_eq!(report["status"], "incomplete");
}

#[test]
fn matching_non_bsc_rpc_propagates_verified_identity_without_endpoint_or_credentials() {
    for (name, id) in [("mainnet", 1), ("base", 8453), ("hyperevm", 999), ("arc-test", 12345)] {
        let network = configured(name, id).selected().unwrap();
        let rpc = ChainRpc::new(json!(format!("{id:#x}")));
        let mut report = json!({});
        network.verify_rpc(&rpc, &mut report).unwrap();
        ensure_finalized(&rpc, 256).unwrap();
        assert_eq!(
            report,
            json!({"network":name,"expected_chain_id":id,"rpc_chain_id":id,"chain_id":id,"network_binding":"rpc_chain_id_verified"})
        );
        assert_eq!(*rpc.calls.lock().unwrap(), ["eth_chainId", "eth_getBlockByNumber"]);
    }
}

#[test]
fn malformed_and_rpc_error_chain_responses_fail_without_sensitive_response_text() {
    let network = configured("base", 8453).selected().unwrap();
    for value in [
        Value::Null,
        json!("https://private.example/token-secret"),
        json!("0xnot-a-chain-secret"),
        json!("0x10000000000000000"),
    ] {
        let rpc = ChainRpc::new(value);
        let mut report = json!({"chain_id":56});
        let error = network.verify_rpc(&rpc, &mut report).unwrap_err().to_string();
        assert!(!error.contains("secret") && !error.contains("private.example"));
        assert!(report.get("chain_id").is_none());
    }
    struct RemoteError;
    impl Rpc for RemoteError {
        fn request(&self, _: Value) -> Result<Value> {
            Ok(json!({"id":1,"error":{"message":"https://private.example/access-secret bearer-secret"}}))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("failed");
    assert!(!record_run(&output, json!({}), |report| network.verify_rpc(&RemoteError, report)).unwrap());
    let saved = fs::read_to_string(output.join("report.json")).unwrap();
    assert!(!saved.contains("secret") && !saved.contains("private.example"));
    assert!(saved.contains("RPC eth_chainId returned error/null"));
}

#[test]
fn legacy_bsc_defaults_are_explicit_but_other_networks_cannot_inherit_endpoints() {
    let bsc = VerificationNetwork::default().selected().unwrap();
    assert_eq!(
        bsc,
        Network {
            name: "bsc".into(),
            chain_id: 56
        }
    );
    let mut report = json!({});
    bsc.verify_rpc(&ChainRpc::new(json!("0x38")), &mut report).unwrap();
    assert_eq!(report["network"], "bsc");
    assert_eq!(report["expected_chain_id"], 56);
    assert_eq!(bsc.rpc_url(None).unwrap(), "https://bsc.rpc.pinax.network");
    assert_eq!(bsc.endpoint(None, false).unwrap(), "bsc.substreams.pinax.network:443");
    assert_eq!(bsc.endpoint(None, true).unwrap(), "bsc.firehose.pinax.network:443");
    let base = configured("base", 8453).selected().unwrap();
    assert!(base.rpc_url(None).is_err());
    assert!(base.endpoint(None, false).is_err());
    assert!(base.endpoint(None, true).is_err());
    assert_eq!(
        base.rpc_url(Some("https://private.example/access-secret".into())).unwrap(),
        "https://private.example/access-secret"
    );
    assert_eq!(base.endpoint(Some("base.example:443"), false).unwrap(), "base.example:443");
    for endpoint in [
        "https://user:secret@example.org",
        "host:443/path-secret",
        "host:443?api-key=secret",
        "host:443#secret",
    ] {
        let error = base.endpoint(Some(endpoint), false).unwrap_err().to_string();
        assert!(!error.contains("secret"));
    }
    assert!(configured("bsc", 1).selected().is_err());
    assert!(configured("base", 0).selected().is_err());
    assert!(configured("https://private.example/secret", 1).selected().is_err());
}

#[test]
fn streaming_credentials_are_never_inherited_by_other_rpc_destinations() {
    let streaming = Some("streaming-test-credential".to_string());
    for url in [
        "https://bsc.rpc.pinax.network",
        "https://bsc.rpc.pinax.network/",
        "https://bsc.rpc.pinax.network:443",
        "https://bsc.rpc.pinax.network:443/",
    ] {
        assert_eq!(rpc_key(url, None, streaming.clone()), streaming);
    }
    for url in [
        "https://other.example",
        "http://bsc.rpc.pinax.network",
        "https://bsc.rpc.pinax.network:8443",
        "https://bsc.rpc.pinax.network.other.example",
        "https://user@bsc.rpc.pinax.network",
        "https://bsc.rpc.pinax.network@other.example",
        "https://bsc.rpc.pinax.network/path",
        "https://bsc.rpc.pinax.network?key=value",
        "https://bsc.rpc.pinax.network#fragment",
    ] {
        assert!(rpc_key(url, None, streaming.clone()).is_none());
        let explicit = Some("explicit-rpc-test-credential".to_string());
        assert_eq!(rpc_key(url, explicit.clone(), streaming.clone()), explicit);
    }
    let explicit = Some("explicit-rpc-test-credential".to_string());
    assert_eq!(rpc_key("https://bsc.rpc.pinax.network", explicit.clone(), streaming), explicit);
}

fn verification(command: &Commands) -> &VerificationNetwork {
    match command {
        Commands::RankTokens(a) => &a.verification,
        Commands::CaptureBlocks(a) => &a.verification,
        Commands::TestRanked(a) => &a.verification,
        Commands::InspectBalance(a) => &a.verification,
        Commands::InspectRanked(a) => &a.verification,
        Commands::RecheckRpc(a) => &a.verification,
        Commands::HolderCoverage(a) => &a.verification,
        Commands::Compare(a) => &a.range.verification,
        Commands::AuditRpc(a) => &a.range.verification,
        Commands::ProbeErc20(a) => &a.verification,
        Commands::RefusalScan(a) => &a.verification,
        Commands::RuntimeStatus(a) => &a.verification,
        _ => panic!("offline package/role inspection does not use RPC identity"),
    }
}

#[test]
fn every_rpc_command_and_offline_refusal_scan_accept_the_same_paired_configuration() {
    for args in [
        vec!["rank-tokens"],
        vec!["capture-blocks", "--start", "1"],
        vec!["test-ranked", "--ranking", "r", "--block-dir", "b"],
        vec!["inspect-balance", "--contract", "c", "--address", "a", "--balance-slot", "s", "--block", "1"],
        vec!["inspect-ranked", "--survey", "s"],
        vec!["recheck-rpc", "--checks", "c"],
        vec!["holder-coverage", "--ranking", "r", "--block-dir", "b", "--layouts", "l"],
        vec!["compare", "--start", "1", "--layouts", "l"],
        vec!["audit-rpc", "--start", "1", "--layouts", "l"],
        vec!["probe-erc20", "--block-file", "b"],
        vec!["refusal-scan", "--block-dir", "b", "--layouts", "l"],
        vec!["runtime-status", "--start", "1", "--layouts", "l"],
    ] {
        let mut command = vec!["tools"];
        command.extend(args);
        command.extend(["--output", "out"]);
        let default = Cli::try_parse_from(&command).unwrap();
        assert!(verification(&default.command).selected().unwrap().is_bsc());
        let mut partial = command.clone();
        partial.extend(["--network", "base"]);
        assert!(Cli::try_parse_from(&partial).is_err());
        let mut partial = command.clone();
        partial.extend(["--expected-chain-id", "8453"]);
        assert!(Cli::try_parse_from(&partial).is_err());
        command.extend(["--network", "base", "--expected-chain-id", "8453"]);
        let selected = Cli::try_parse_from(&command).unwrap();
        assert_eq!(
            verification(&selected.command).selected().unwrap(),
            configured("base", 8453).selected().unwrap()
        );
    }
}

#[test]
fn input_reports_and_source_metadata_must_match_the_selected_chain() {
    let base = configured("base", 8453).selected().unwrap();
    base.check_report(&json!({"network":"base","chain_id":8453,"rpc_chain_id":8453,"expected_chain_id":8453}))
        .unwrap();
    for wrong in [
        json!({"chain_id":56}),
        json!({"expected_chain_id":56}),
        json!({"rpc_chain_id":56}),
        json!({"network":"bsc","chain_id":8453}),
    ] {
        assert!(base.check_report(&wrong).is_err());
    }
    // Legacy files without identity are not re-labelled as verified by this check.
    let legacy = json!({"hash":"historical-control"});
    base.check_report(&legacy).unwrap();
    assert!(legacy.get("chain_id").is_none());
    assert!(base.check_ranking(&json!({"status":"ranked"})).is_err());
    assert!(base.check_ranking(&json!({"status":"incomplete","chain_id":8453})).is_err());
    assert!(base.check_ranking(&json!({"status":"ranked","chain_id":56})).is_err());
    base.check_ranking(&json!({"status":"ranked","chain_id":8453})).unwrap();
    for chain in [json!(8453), json!("8453")] {
        base.check_source_chain(&json!({"chainId":chain})).unwrap();
    }
    for chain in [json!(56), json!("56"), json!("private-endpoint-secret"), Value::Null] {
        let error = base.check_source_chain(&json!({"chainId":chain})).unwrap_err().to_string();
        assert!(!error.contains("secret"));
    }
}

#[test]
fn stream_checks_chain_before_opening_package_or_launching_any_external_command() {
    let network = configured("base", 8453).selected().unwrap();
    let stream = capture::Stream {
        network: &network,
        start: 1,
        blocks: 2,
        endpoint: "base.example:443",
        timeout: 1,
        package: Path::new("nonexistent-package.spkg"),
        params: None,
    };
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("events.jsonl");
    let rpc = ChainRpc::new(json!("0x38"));
    let error = capture::stream_events(&stream, &output, &rpc).unwrap_err().to_string();
    assert!(error.contains("RPC chain ID mismatch"));
    assert!(!output.exists());
    assert_eq!(*rpc.calls.lock().unwrap(), ["eth_chainId"]);
    let command = capture::stream_command(&stream, "clock").unwrap();
    let arguments: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
    assert!(arguments.windows(2).any(|a| a == ["--network", "base"]));
    assert!(arguments.windows(2).any(|a| a == ["-e", "base.example:443"]));
}

#[test]
fn offline_refusal_scan_records_caller_configuration_without_claiming_verified_chain() {
    let temp = tempfile::tempdir().unwrap();
    let block_dir = temp.path().join("blocks");
    fs::create_dir(&block_dir).unwrap();
    fs::write(block_dir.join("122260950.pb"), include_bytes!("../../../tests/fixtures/bsc-122260950.pb")).unwrap();
    let layouts = temp.path().join("layouts.json");
    fs::write(&layouts, "[]").unwrap();
    let output = temp.path().join("scan");
    assert!(crate::refusal_scan::run(crate::refusal_scan::RefusalScan {
        verification: VerificationNetwork::default(),
        layouts,
        block_dir,
        allow_gaps: false,
        output: output.clone(),
    })
    .unwrap());
    let report: Value = serde_json::from_slice(&fs::read(output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["network"], "bsc");
    assert_eq!(report["expected_chain_id"], 56);
    assert_eq!(report["network_binding"], "caller_configured_offline_blocks_have_no_chain_id");
    assert!(report.get("chain_id").is_none() && report.get("rpc_chain_id").is_none());
}
