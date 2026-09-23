//! Producer behavior on actual AccessControl role operations. For each
//! `RoleGranted`/`RoleRevoked` log in captured Extended blocks, the frame's
//! storage writes to the emitting contract are classified through the block's
//! Keccak preimages: an enumerable set's length, element or member index, a
//! plain membership flag, a role admin, or unclassified. The report keeps each
//! write's ordinal and whether it kept its value. Diagnostic only: nothing
//! here configures or qualifies a rule.
use crate::cli::record_run;
use anyhow::{Context, Result};
use clap::Args;
use erc20_balances::hash;
use prost::Message;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};
use substreams_ethereum::pb::eth::v2 as eth;

#[derive(Args)]
pub struct RoleOperations {
    /// Captured Extended blocks, one `<number>.pb` file each; need not be consecutive.
    #[arg(long)]
    pub block_dir: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

type Word = [u8; 32];

fn word(bytes: &[u8]) -> Word {
    let mut out = [0; 32];
    let bytes = &bytes[bytes.len().saturating_sub(32)..];
    out[32 - bytes.len()..].copy_from_slice(bytes);
    out
}
fn add(base: &Word, offset: u64) -> Word {
    let mut out = *base;
    let mut carry = offset as u128;
    for byte in out.iter_mut().rev() {
        let sum = *byte as u128 + (carry & 0xff);
        *byte = sum as u8;
        carry = (carry >> 8) + (sum >> 8);
    }
    out
}
/// `key - base` when it is below 2^32, the span of a plausible array.
fn offset(key: &Word, base: &Word) -> Option<u64> {
    let (mut diff, mut borrow) = ([0u8; 32], 0i16);
    for i in (0..32).rev() {
        let d = key[i] as i16 - base[i] as i16 - borrow;
        borrow = i16::from(d < 0);
        diff[i] = (d + 256 * borrow) as u8;
    }
    (borrow == 0 && diff[..28].iter().all(|b| *b == 0)).then(|| u32::from_be_bytes(diff[28..].try_into().unwrap()) as u64)
}
fn hex0x(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn address(word: &[u8]) -> Option<String> {
    (word.len() == 32 && word[..12] == [0; 12]).then(|| hex0x(&word[12..]))
}

/// OpenZeppelin 5 upgradeable `AccessControl` and `AccessControlEnumerable`
/// ERC-7201 namespaces, tried as parent slots when the block has no preimage.
const NAMESPACES: [&str; 2] = [
    "02dd7bc7dec4dceedda775e58dd541e08a116c6c53815c0bd028192f7b626800",
    "c1f6fe24621ce81ec5827caf0253cadb74709b061630e6b55e82371705932000",
];

/// Candidate roots `keccak(role || S)` for `role`, each with its parent slot
/// and whether the block recorded the preimage. With a constant role the
/// compiler may fold that hash, so slots 0..=255 and the OpenZeppelin
/// namespaces are also computed.
fn roots(role: &Word, preimages: &HashMap<Word, Vec<u8>>) -> BTreeMap<Word, (String, &'static str)> {
    let mut roots = preimages
        .iter()
        .filter(|(_, p)| p.len() == 64 && p[..32] == role[..])
        .map(|(h, p)| (*h, (hex0x(&p[32..]), "block_preimage")))
        .collect::<BTreeMap<_, _>>();
    let parents = (0u8..=255).map(|s| word(&[s])).chain(NAMESPACES.iter().map(|n| word(&hex::decode(n).unwrap())));
    for parent in parents {
        let root = hash(&[&role[..], &parent[..]].concat());
        roots.entry(root).or_insert((hex0x(&parent), "computed"));
    }
    roots
}

/// The field a key names for `role`; an array's data starts at `keccak(root)`.
fn field(key: &Word, roots: &BTreeMap<Word, (String, &'static str)>, preimages: &HashMap<Word, Vec<u8>>) -> Value {
    let mapping = preimages.get(key).filter(|p| p.len() == 64);
    for (root, (parent, source)) in roots {
        let named = |field: &str| json!({"field":field,"parent_slot":parent,"root_source":source});
        if key == root {
            return named("root");
        }
        for (offset, name) in [(1, "root+1"), (2, "root+2")] {
            if *key == add(root, offset) {
                return named(name);
            }
        }
        if let Some(p) = mapping {
            let base = word(&p[32..]);
            for (name, expected) in [("member_of_root", *root), ("index_at_root+1", add(root, 1))] {
                if base == expected {
                    let mut row = named(name);
                    row["member"] = json!(address(&p[..32]));
                    return row;
                }
            }
        }
        if let Some(position) = offset(key, &hash(root)) {
            let mut row = named("element");
            row["position"] = json!(position);
            return row;
        }
    }
    match preimages.get(key) {
        Some(p) if p.len() == 64 => json!({"field":"unclassified","mapping_key":hex0x(&p[..32]),"base":hex0x(&p[32..])}),
        _ => json!({"field":"unclassified"}),
    }
}

pub fn classify(block: &eth::Block) -> Result<Vec<Value>> {
    let granted = hash(b"RoleGranted(bytes32,address,address)");
    let revoked = hash(b"RoleRevoked(bytes32,address,address)");
    let mut preimages = HashMap::new();
    for call in block.transaction_traces.iter().flat_map(|t| &t.calls) {
        for (h, p) in &call.keccak_preimages {
            let h = hex::decode(h.trim_start_matches("0x"))?;
            let p = hex::decode(p.trim_start_matches("0x"))?;
            if h.len() == 32 {
                preimages.insert(word(&h), p);
            }
        }
    }
    let mut operations = Vec::new();
    for trace in &block.transaction_traces {
        for call in &trace.calls {
            let logs = call
                .logs
                .iter()
                .filter(|l| l.topics.len() == 4 && (l.topics[0] == granted || l.topics[0] == revoked))
                .collect::<Vec<_>>();
            for log in &logs {
                let role = word(&log.topics[1]);
                let roots = roots(&role, &preimages);
                // Operations on the same role in one frame share their writes.
                let shared = logs.iter().filter(|l| l.address == log.address && l.topics[1] == log.topics[1]).count();
                let writes = call
                    .storage_changes
                    .iter()
                    .filter(|c| c.address == log.address)
                    .map(|c| {
                        let mut row = field(&word(&c.key), &roots, &preimages);
                        row["ordinal"] = json!(c.ordinal);
                        row["key"] = json!(hex0x(&c.key));
                        row["old"] = json!(hex0x(&c.old_value));
                        row["new"] = json!(hex0x(&c.new_value));
                        row["unchanged"] = json!(word(&c.old_value) == word(&c.new_value));
                        row
                    })
                    .collect::<Vec<_>>();
                let ordinals = writes.iter().map(|w| w["ordinal"].as_u64().unwrap()).collect::<Vec<_>>();
                let role_writes = writes.iter().filter(|w| w["field"] != "unclassified").collect::<Vec<_>>();
                operations.push(json!({
                    "block":block.number,"transaction":hex0x(&trace.hash),"transaction_status":trace.status,
                    "call_index":call.index,"persisted":!call.state_reverted,"contract":hex0x(&log.address),
                    "kind":if log.topics[0] == granted {"grant"} else {"revoke"},"role":hex0x(&role),
                    "account":address(&log.topics[2]),"log_ordinal":log.ordinal,"operations_on_role_in_frame":shared,
                    "shape":role_writes.iter().map(|w| w["field"].clone()).collect::<Vec<_>>(),
                    "unchanged_writes":writes.iter().filter(|w| w["unchanged"] == true).count(),
                    "ordinals_strictly_increasing":ordinals.windows(2).all(|w| w[0] < w[1]),
                    "writes":writes,
                }));
            }
        }
    }
    Ok(operations)
}

pub fn run(args: RoleOperations) -> Result<bool> {
    record_run(
        &args.output,
        json!({"status":"incomplete","scope":"Storage writes in frames that emit RoleGranted/RoleRevoked; diagnostic, no rule is configured"}),
        |report| {
            let files = crate::refusal_scan::block_files(&args.block_dir)?;
            let mut operations = Vec::new();
            let mut blocks = Vec::new();
            for (number, path) in &files {
                let block = eth::Block::decode(fs::read(path)?.as_slice())?;
                anyhow::ensure!(block.number == *number, "{} holds block {}", path.display(), block.number);
                anyhow::ensure!(
                    block.detail_level == eth::block::DetailLevel::DetaillevelExtended as i32,
                    "Extended block required at {number}"
                );
                blocks.push(json!({"block":number,"hash":hex0x(&block.hash),"producer_version":block.ver}));
                operations.extend(classify(&block)?);
            }
            let count = |f: &dyn Fn(&Value) -> bool| operations.iter().filter(|o| f(o)).count();
            let mut shapes = BTreeMap::<String, u64>::new();
            for o in operations.iter().filter(|o| o["persisted"] == true && o["operations_on_role_in_frame"] == 1) {
                let shape = o["shape"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect::<Vec<_>>().join(",");
                *shapes.entry(format!("{} [{}]", o["kind"].as_str().unwrap(), shape)).or_default() += 1;
            }
            report["blocks"] = json!(blocks);
            report["operations"] = json!(operations.len());
            report["persisted"] = json!(count(&|o| o["persisted"] == true));
            report["grants"] = json!(count(&|o| o["kind"] == "grant"));
            report["revokes"] = json!(count(&|o| o["kind"] == "revoke"));
            report["unchanged_writes"] = json!(operations.iter().map(|o| o["unchanged_writes"].as_u64().unwrap()).sum::<u64>());
            report["frames_with_non_increasing_ordinals"] = json!(count(&|o| o["ordinals_strictly_increasing"] != true));
            report["single_operation_shapes"] = json!(shapes);
            let path = args.output.join("operations.json");
            fs::write(&path, serde_json::to_string_pretty(&operations)?)?;
            report["operations_sha256"] = json!(crate::data::sha256(&path)?);
            report["status"] = json!("inspected");
            Ok(())
        },
    )
    .context("role operations")
}
