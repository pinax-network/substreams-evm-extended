//! Inputs replayed through both packaged `db_out` modules.
//!
//! The fixture case pairs the native reducer's output for the committed
//! Extended BSC block with the ERC-20 rows of the saved RPC reference. Synthetic cases
//! reach the branches real BSC data does not: genesis clocks, Tron encoding,
//! invalid parameters, missing timestamps and inputs that each table skips.

use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use erc20_balances::layout::VerifiedLayout;
use prost::Message;
use proto::pb::evm::balances::v1 as pb;
use serde::Deserialize;
use substreams::pb::substreams::Clock;
use substreams_ethereum::pb::eth::v2 as eth;

/// The native map's committed parameters.
pub const NATIVE_PARAMS: &str = r#"{"producer_versions":[5]}"#;

/// The committed complete BSC v5 block, relative to the repository root.
pub const FIXTURE_BLOCK: &str = "erc20/balances/tests/fixtures/bsc-122260950.pb";

/// Same-block `eth_getBalance` / `balanceOf` reference rows for that block.
pub const RPC_REFERENCE: &str = "erc20/balances/tests/fixtures/bsc-122260950.json";

#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub params: String,
    pub clock: Clock,
    pub native: pb::Events,
    pub erc20: pb::Events,
}

impl Case {
    /// `db_out` inputs in manifest order: params, clock, native, ERC-20.
    pub fn inputs(&self) -> [Vec<u8>; 4] {
        [
            self.params.clone().into_bytes(),
            self.clock.encode_to_vec(),
            self.native.encode_to_vec(),
            self.erc20.encode_to_vec(),
        ]
    }
}

pub fn clock_of(block: &eth::Block) -> Result<Clock> {
    let header = block.header.as_ref().ok_or_else(|| anyhow!("block {} has no header", block.number))?;
    Ok(Clock {
        id: hex::encode(&block.hash),
        number: block.number,
        timestamp: header.timestamp,
    })
}

/// Layouts for the Extended ERC-20 map, as raw JSON (for package checks) and parsed.
pub struct Erc20Layouts {
    pub json: serde_json::Value,
    pub parsed: Vec<VerifiedLayout>,
}

pub fn read_layouts(path: &Path) -> Result<Erc20Layouts> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Erc20Layouts {
        json: serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?,
        parsed: erc20_balances::layout::parse(&text).map_err(|e| anyhow!("{}: {e}", path.display()))?,
    })
}

/// The Extended ERC-20 map's output for `block` with these layouts.
pub fn erc20_events(block: &eth::Block, layouts: &[VerifiedLayout]) -> Result<pb::Events> {
    erc20_balances::project(block, layouts).map_err(|e| anyhow!("ERC-20 map refused block {}: {e}", block.number))
}

/// The native map's output for `block`, as the packaged module computes it.
pub fn native_events(block: &eth::Block) -> Result<pb::Events> {
    let params = native_balances::parse_params(NATIVE_PARAMS).map_err(|e| anyhow!("{e}"))?;
    native_balances::project(block, &params).map_err(|e| anyhow!("native reducer refused block {}: {e}", block.number))
}

pub fn read_block(path: &Path) -> Result<eth::Block> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    eth::Block::decode(bytes.as_slice()).with_context(|| format!("decoding {}", path.display()))
}

#[derive(Deserialize)]
struct RpcReference {
    block_number: u64,
    rpc_checks: Vec<RpcRow>,
}

#[derive(Deserialize)]
struct RpcRow {
    contract: String,
    address: String,
    balance: String,
}

/// ERC-20 rows of the saved RPC reference, as the RPC `erc20_balances` package emits them.
fn rpc_erc20_events(path: &Path) -> Result<(u64, pb::Events)> {
    let reference: RpcReference = serde_json::from_slice(&fs::read(path).with_context(|| format!("reading {}", path.display()))?)?;
    let balances = reference
        .rpc_checks
        .iter()
        .filter(|row| !row.contract.is_empty())
        .map(|row| {
            Ok(pb::Balance {
                contract: Some(hex_bytes(&row.contract)?),
                address: hex_bytes(&row.address)?,
                amount: row.balance.clone(),
            })
        })
        .collect::<Result<_>>()?;
    Ok((reference.block_number, pb::Events { balances }))
}

/// The complete captured block with its native map output and the RPC
/// reference's ERC-20 rows. The many single-transaction captures are trimmed
/// blocks the native reducer refuses; [`block_dir_cases`] reports them.
pub fn fixture_case(root: &Path) -> Result<Case> {
    let (rpc_block, erc20) = rpc_erc20_events(&root.join(RPC_REFERENCE))?;
    let block = read_block(&root.join(FIXTURE_BLOCK))?;
    if block.number != rpc_block {
        bail!("RPC reference is for block {rpc_block}, fixture is block {}", block.number);
    }
    Ok(Case {
        name: FIXTURE_BLOCK.to_string(),
        params: "hex".into(),
        clock: clock_of(&block)?,
        native: native_events(&block)?,
        erc20,
    })
}

/// A file that did not become a case, with the reason.
pub type Refused = (PathBuf, String);

/// The complete captured block with both RPC-free inputs: the native map and
/// the Extended ERC-20 map with `layouts`.
pub fn extended_fixture_case(root: &Path, layouts: &[VerifiedLayout]) -> Result<Case> {
    let block = read_block(&root.join(FIXTURE_BLOCK))?;
    Ok(Case {
        name: format!("{FIXTURE_BLOCK} (Extended ERC-20)"),
        params: "hex".into(),
        clock: clock_of(&block)?,
        native: native_events(&block)?,
        erc20: erc20_events(&block, layouts)?,
    })
}

/// Every `*.pb` under `dir`, with the Extended ERC-20 map's output when
/// `erc20` layouts are given. Files that are not blocks, or blocks a map
/// refuses, are returned separately with the reason rather than replaced by
/// an empty input.
pub fn block_dir_cases(dir: &Path, erc20: Option<&[VerifiedLayout]>) -> Result<(Vec<Case>, Vec<Refused>)> {
    let (mut cases, mut refused) = (Vec::new(), Vec::new());
    for path in sorted_pb(dir)? {
        let case = read_block(&path).and_then(|block| {
            Ok(Case {
                name: path.display().to_string(),
                params: "hex".into(),
                clock: clock_of(&block)?,
                native: native_events(&block)?,
                erc20: erc20.map(|layouts| erc20_events(&block, layouts)).transpose()?.unwrap_or_default(),
            })
        });
        match case {
            Ok(case) => cases.push(case),
            Err(e) => refused.push((path, format!("{e:#}"))),
        }
    }
    Ok((cases, refused))
}

/// Every `*.pb` under `dir`, sorted by path.
pub fn sorted_pb(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_pb(dir, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_pb(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_pb(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "pb") {
            out.push(path);
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct JsonlLine {
    #[serde(rename = "@block")]
    block: u64,
    #[serde(rename = "@data", default)]
    data: JsonlEvents,
}

#[derive(Deserialize, Default)]
struct JsonlEvents {
    #[serde(default)]
    balances: Vec<JsonlBalance>,
}

#[derive(Deserialize)]
struct JsonlBalance {
    contract: Option<String>,
    address: String,
    amount: String,
}

/// `map_events` output recorded with `substreams run -o jsonl --bytes-encoding
/// hex`, by block. Blocks without output are absent.
pub fn read_jsonl_events(path: &Path) -> Result<Vec<(u64, pb::Events)>> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut out = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line: JsonlLine = serde_json::from_str(&line?).with_context(|| format!("{}:{}", path.display(), index + 1))?;
        let balances = line
            .data
            .balances
            .iter()
            .map(|b| {
                Ok(pb::Balance {
                    contract: b.contract.as_deref().map(hex_bytes).transpose()?,
                    address: hex_bytes(&b.address)?,
                    amount: b.amount.clone(),
                })
            })
            .collect::<Result<_>>()?;
        out.push((line.block, pb::Events { balances }));
    }
    Ok(out)
}

/// Native `map_events` recordings. They carry no clock, so each case gets a
/// SYNTHETIC one (hash = sha256 of the number, 450 ms cadence); both packages
/// receive the same clock.
pub fn jsonl_cases(path: &Path) -> Result<Vec<Case>> {
    Ok(read_jsonl_events(path)?
        .into_iter()
        .map(|(block, native)| Case {
            name: format!("{}@{block}", path.display()),
            params: "hex".into(),
            clock: synthetic_clock(block),
            native,
            erc20: pb::Events::default(),
        })
        .collect())
}

pub fn synthetic_clock(number: u64) -> Clock {
    use sha2::{Digest, Sha256};
    let seconds = 1_790_121_600 + (number % 1_000_000) as i64 * 45 / 100;
    Clock {
        id: hex::encode(Sha256::digest(number.to_be_bytes())),
        number,
        timestamp: Some(prost_types::Timestamp { seconds, nanos: 0 }),
    }
}

/// `(contract, address, amount)` of a synthetic ERC-20 row.
type Erc20Row<'a> = (Option<Vec<u8>>, Vec<u8>, &'a str);

/// Branches of `db_out` that real BSC v5 blocks do not reach.
pub fn synthetic_cases() -> Vec<Case> {
    let address = |byte: u8| vec![byte; 20];
    let native = |rows: &[(Vec<u8>, &str)]| pb::Events {
        balances: rows
            .iter()
            .map(|(a, amount)| pb::Balance {
                contract: None,
                address: a.clone(),
                amount: (*amount).into(),
            })
            .collect(),
    };
    let erc20 = |rows: &[Erc20Row<'_>]| pb::Events {
        balances: rows
            .iter()
            .map(|(c, a, amount)| pb::Balance {
                contract: c.clone(),
                address: a.clone(),
                amount: (*amount).into(),
            })
            .collect(),
    };
    let case = |name: &str, params: &str, clock: Clock, n: pb::Events, e: pb::Events| Case {
        name: name.into(),
        params: params.into(),
        clock,
        native: n,
        erc20: e,
    };
    let no_timestamp = |number: u64, id: &str| Clock {
        id: id.into(),
        number,
        timestamp: None,
    };
    let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";

    let mut cases = vec![
        case("empty inputs", "hex", synthetic_clock(1), pb::Events::default(), pb::Events::default()),
        case(
            "empty params default to hex",
            "",
            synthetic_clock(2),
            native(&[(address(1), "1")]),
            pb::Events::default(),
        ),
        case(
            "zero, uint256 max and unparsed amounts",
            "hex",
            synthetic_clock(3),
            native(&[(address(1), "0"), (address(2), max), (address(3), "not-a-number")]),
            erc20(&[(Some(address(9)), address(1), "0"), (Some(address(9)), address(2), max)]),
        ),
        case(
            "native rows ignore contract; ERC-20 rows need one",
            "hex",
            synthetic_clock(4),
            pb::Events {
                balances: vec![pb::Balance {
                    contract: Some(address(7)),
                    address: address(1),
                    amount: "5".into(),
                }],
            },
            erc20(&[(None, address(2), "6"), (Some(vec![]), address(3), "7"), (Some(address(8)), vec![], "8")]),
        ),
        case(
            "repeated keys within one block",
            "hex",
            synthetic_clock(5),
            native(&[(address(1), "1"), (address(1), "2")]),
            erc20(&[(Some(address(9)), address(1), "3"), (Some(address(9)), address(1), "4")]),
        ),
        case(
            "tron base58 with 20, 21 and malformed address lengths",
            "tron_base58",
            synthetic_clock(6),
            native(&[
                (address(1), "1"),
                ([vec![0x41], address(2)].concat(), "2"),
                ([vec![0x42], address(3)].concat(), "3"),
                (vec![4; 19], "4"),
            ]),
            erc20(&[(Some(address(9)), address(5), "5")]),
        ),
        case(
            "invalid params",
            "base64",
            synthetic_clock(7),
            native(&[(address(1), "1")]),
            pb::Events::default(),
        ),
        case(
            "missing timestamp",
            "hex",
            no_timestamp(8, "ab"),
            native(&[(address(1), "1")]),
            pb::Events::default(),
        ),
        case(
            "missing timestamp without rows",
            "hex",
            no_timestamp(9, "ab"),
            pb::Events::default(),
            pb::Events::default(),
        ),
        case(
            "unknown genesis without timestamp",
            "hex",
            no_timestamp(0, "00"),
            native(&[(address(1), "1")]),
            pb::Events::default(),
        ),
    ];
    for (network, id) in [
        ("Ethereum", "d4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"),
        ("Arbitrum One", "7ee576b35482195fc49205cec9af72ce14f003b9ae69f6ba0faef4514be8b442"),
        ("Arbitrum Nova", "2ad24e03026118f9b3a48626f0636e38c93660e90a6812e853a99aa8c5371561"),
        ("Boba", "dcd9e6a8f9973eaa62da2874959cb152faeb4fd6929177bd6335a1a16074ef9c"),
    ] {
        cases.push(case(
            &format!("{network} genesis clock"),
            "hex",
            no_timestamp(0, id),
            native(&[(address(1), "1")]),
            pb::Events::default(),
        ));
    }
    cases
}

fn hex_bytes(value: &str) -> Result<Vec<u8>> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    if digits.len() % 2 != 0 {
        bail!("odd-length hex {value:?}");
    }
    hex::decode(digits).with_context(|| format!("invalid hex {value:?}"))
}
