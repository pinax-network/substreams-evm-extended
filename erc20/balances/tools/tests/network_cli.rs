#![cfg(not(target_arch = "wasm32"))]

use erc20_balances_tools::data::sha256;
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

/// Exercise every RPC entry point through its real CLI, with a loopback-only
/// server. The server permits exactly the identity request: a header, runtime,
/// balance or capture request before refusing the chain is a test failure.
#[test]
fn all_rpc_commands_refuse_wrong_chain_before_data_requests_and_preserve_safe_reports() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = temp.path().join("inputs");
    let blocks = inputs.join("blocks");
    fs::create_dir_all(&blocks).unwrap();
    fs::write(blocks.join("122260950.pb"), include_bytes!("../../tests/fixtures/bsc-122260950.pb")).unwrap();
    let layouts = inputs.join("layouts.json");
    fs::write(&layouts, include_bytes!("../../tests/fixtures/verified-layouts.json")).unwrap();
    let contract = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
    let holder = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let reference = inputs.join("reference.jsonl");
    write(
        &reference,
        &json!({"@module":"map_events","@block":122260950,"@type":"evm.balances.v1.Events",
        "@data":{"balances":[{"contract":contract,"address":holder,"amount":"1"}]}}),
    );
    let ranking = inputs.join("ranking.json");
    write(
        &ranking,
        &json!({"status":"ranked","network":"base","chain_id":8453,"start":122260950,
        "stop_exclusive":122260951,"reference_sha256":sha256(&reference).unwrap(),"selected_tokens":[]}),
    );
    let checks = inputs.join("rpc-checks.jsonl");
    write(
        &checks,
        &json!({"rpc":null,"match":false,"block":122260950,"boundary":"after",
        "contract":contract,"address":holder,"storage":"1","hash":format!("0x{}", "01".repeat(32))}),
    );
    let survey = inputs.join("survey.json");
    write(
        &survey,
        &json!({"network":"base","chain_id":8453,"investigation_complete":true,
        "rpc_checks_sha256":sha256(&checks).unwrap(),"tokens":[]}),
    );
    write(&inputs.join("report.json"), &json!({"network":"base","chain_id":8453}));
    let p = |path: &Path| path.to_str().unwrap().to_string();
    let commands = [
        vec!["rank-tokens".into()],
        vec!["capture-blocks".into(), "--start".into(), "1".into()],
        vec!["compare".into(), "--start".into(), "1".into(), "--layouts".into(), p(&layouts)],
        vec!["audit-rpc".into(), "--start".into(), "1".into(), "--layouts".into(), p(&layouts)],
        vec!["runtime-status".into(), "--start".into(), "1".into(), "--layouts".into(), p(&layouts)],
        vec![
            "inspect-balance".into(),
            "--contract".into(),
            contract.into(),
            "--address".into(),
            holder.into(),
            "--balance-slot".into(),
            format!("0x{}", "00".repeat(32)),
            "--block".into(),
            "1".into(),
        ],
        vec!["probe-erc20".into(), "--block-file".into(), p(&blocks.join("122260950.pb"))],
        vec!["test-ranked".into(), "--ranking".into(), p(&ranking), "--block-dir".into(), p(&blocks)],
        vec![
            "holder-coverage".into(),
            "--ranking".into(),
            p(&ranking),
            "--block-dir".into(),
            p(&blocks),
            "--layouts".into(),
            p(&layouts),
        ],
        vec!["inspect-ranked".into(), "--survey".into(), p(&survey)],
        vec!["recheck-rpc".into(), "--checks".into(), p(&checks)],
    ];
    for mut args in commands {
        let name = args[0].clone();
        let output = temp.path().join(&name);
        args.extend([
            "--network".into(),
            "base".into(),
            "--expected-chain-id".into(),
            "8453".into(),
            "--output".into(),
            p(&output),
        ]);
        if ["rank-tokens", "capture-blocks", "compare", "audit-rpc"].contains(&name.as_str()) {
            args.extend(["--endpoint".into(), "unused.example:443".into()]);
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/endpoint-secret?api-key=query-secret", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                    other => panic!("identity request did not reach loopback server: {other:?}"),
                }
            };
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(request["method"], "eth_chainId");
            let response = json!({"jsonrpc":"2.0","id":request["id"],"result":"0x38"}).to_string();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        });
        let result = Command::new(env!("CARGO_BIN_EXE_erc20-balances-tools"))
            .args(&args)
            .env("RPC_URL", &endpoint)
            .env("RPC_API_KEY", "header-secret")
            .output()
            .unwrap();
        server.join().unwrap();
        assert!(!result.status.success(), "{name} unexpectedly succeeded");
        let saved = fs::read_to_string(output.join("report.json")).unwrap();
        let report: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(report["network"], "base", "{name}");
        assert_eq!(report["expected_chain_id"], 8453, "{name}");
        assert_eq!(report["rpc_chain_id"], 56, "{name}");
        assert_eq!(report["status"], "incomplete", "{name}");
        assert!(report.get("chain_id").is_none(), "{name}");
        assert!(report["failure"].as_str().unwrap().contains("RPC chain ID mismatch"), "{name}: {saved}");
        for evidence in [saved, String::from_utf8(result.stdout).unwrap(), String::from_utf8(result.stderr).unwrap()] {
            assert!(
                !evidence.contains("secret") && !evidence.contains(&endpoint),
                "{name} leaked connection credentials"
            );
        }
    }
}
