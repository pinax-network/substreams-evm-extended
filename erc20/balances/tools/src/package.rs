//! Identity of a built SPKG: its modules with their inputs, output and default
//! parameters, the host functions each WASM binary imports, and the embedded
//! balance schema compared byte for byte with the canonical RPC reference.
use crate::{cli::record_run, data::sha256};
use anyhow::{bail, ensure, Context, Result};
use clap::Args;
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Args)]
pub struct InspectPackage {
    #[arg(long, default_value_os_t = crate::data::default_package())]
    pub package: PathBuf,
    #[arg(long, default_value_os_t = crate::data::default_reference())]
    pub reference: PathBuf,
    /// Optional built WASM expected to be the package's map binary.
    #[arg(long)]
    pub wasm: Option<PathBuf>,
    #[arg(long)]
    pub output: PathBuf,
}

// The subset of `sf.substreams.v1.Package` this check reads.
#[derive(Clone, PartialEq, Message)]
struct Package {
    /// `google.protobuf.FileDescriptorProto`, kept as encoded bytes.
    #[prost(bytes = "vec", repeated, tag = "1")]
    proto_files: Vec<Vec<u8>>,
    #[prost(message, optional, tag = "6")]
    modules: Option<Modules>,
}
#[derive(Clone, PartialEq, Message)]
struct FileName {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    package: String,
}
#[derive(Clone, PartialEq, Message)]
struct Modules {
    #[prost(message, repeated, tag = "1")]
    modules: Vec<Module>,
    #[prost(message, repeated, tag = "2")]
    binaries: Vec<Binary>,
}
#[derive(Clone, PartialEq, Message)]
struct Binary {
    #[prost(string, tag = "1")]
    r#type: String,
    #[prost(bytes = "vec", tag = "2")]
    content: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct Module {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(message, optional, tag = "2")]
    kind_map: Option<KindMap>,
    #[prost(bytes = "vec", optional, tag = "3")]
    kind_store: Option<Vec<u8>>,
    #[prost(uint32, tag = "4")]
    binary_index: u32,
    #[prost(message, repeated, tag = "6")]
    inputs: Vec<Input>,
    #[prost(message, optional, tag = "7")]
    output: Option<Named>,
    #[prost(uint64, tag = "8")]
    initial_block: u64,
    #[prost(bytes = "vec", optional, tag = "10")]
    kind_block_index: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Message)]
struct KindMap {
    #[prost(string, tag = "1")]
    output_type: String,
}
/// `Input` is a oneof; each arm's first field is its type, module or value.
#[derive(Clone, PartialEq, Message)]
struct Input {
    #[prost(message, optional, tag = "1")]
    source: Option<Named>,
    #[prost(message, optional, tag = "2")]
    map: Option<Named>,
    #[prost(message, optional, tag = "3")]
    store: Option<Named>,
    #[prost(message, optional, tag = "4")]
    params: Option<Named>,
}
#[derive(Clone, PartialEq, Message)]
struct Named {
    #[prost(string, tag = "1")]
    value: String,
}

fn leb(bytes: &[u8], at: &mut usize) -> Result<u64> {
    let (mut value, mut shift) = (0u64, 0);
    loop {
        let byte = *bytes.get(*at).context("truncated WASM integer")?;
        *at += 1;
        ensure!(shift < 64, "oversized WASM integer");
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
}
fn name<'a>(bytes: &'a [u8], at: &mut usize) -> Result<&'a str> {
    let len = leb(bytes, at)? as usize;
    let end = at.checked_add(len).filter(|end| *end <= bytes.len()).context("truncated WASM name")?;
    let text = std::str::from_utf8(&bytes[*at..end])?;
    *at = end;
    Ok(text)
}
fn limits(bytes: &[u8], at: &mut usize) -> Result<()> {
    let flags = leb(bytes, at)?;
    leb(bytes, at)?;
    if flags & 1 != 0 {
        leb(bytes, at)?;
    }
    Ok(())
}

/// Every `module.field` a WASM binary imports, in import-section order.
pub fn wasm_imports(wasm: &[u8]) -> Result<Vec<String>> {
    ensure!(wasm.len() >= 8 && wasm[..4] == *b"\0asm" && wasm[4..8] == [1, 0, 0, 0], "not a WASM v1 binary");
    let mut at = 8;
    let mut imports = Vec::new();
    while at < wasm.len() {
        let id = wasm[at];
        at += 1;
        let size = leb(wasm, &mut at)? as usize;
        let end = at.checked_add(size).filter(|end| *end <= wasm.len()).context("truncated WASM section")?;
        if id == 2 {
            let section = &wasm[..end];
            for _ in 0..leb(section, &mut at)? {
                let module = name(section, &mut at)?;
                let field = name(section, &mut at)?;
                let kind = *section.get(at).context("truncated WASM import")?;
                at += 1;
                match kind {
                    0 => {
                        leb(section, &mut at)?;
                    }
                    1 => {
                        at += 1;
                        limits(section, &mut at)?;
                    }
                    2 => limits(section, &mut at)?,
                    3 => at += 2,
                    other => bail!("unknown WASM import kind {other}"),
                }
                imports.push(format!("{module}.{field}"));
            }
            ensure!(at == end, "WASM import section length mismatch");
        }
        at = end;
    }
    Ok(imports)
}

fn decode(path: &PathBuf) -> Result<Package> {
    Package::decode(fs::read(path)?.as_slice()).with_context(|| format!("invalid SPKG {}", path.display()))
}
fn schema(package: &Package, proto_package: &str) -> Result<Vec<u8>> {
    let files = package
        .proto_files
        .iter()
        .filter(|file| FileName::decode(file.as_slice()).is_ok_and(|f| f.package == proto_package))
        .collect::<Vec<_>>();
    ensure!(files.len() == 1, "expected exactly one {proto_package} descriptor");
    Ok(files[0].clone())
}
fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn describe(package: &Package) -> Result<Value> {
    let modules = package.modules.as_ref().context("package has no modules")?;
    let binaries = modules
        .binaries
        .iter()
        .map(|binary| Ok(json!({"type":binary.r#type,"sha256":digest(&binary.content),"imports":wasm_imports(&binary.content)?})))
        .collect::<Result<Vec<_>>>()?;
    let modules = modules
        .modules
        .iter()
        .map(|module| {
            let kind = match (&module.kind_map, &module.kind_store, &module.kind_block_index) {
                (Some(_), None, None) => "map",
                (None, Some(_), None) => "store",
                (None, None, Some(_)) => "block_index",
                _ => bail!("module {} has no single kind", module.name),
            };
            let inputs = module
                .inputs
                .iter()
                .map(|input| match (&input.source, &input.map, &input.store, &input.params) {
                    (Some(s), None, None, None) => Ok(json!({"source":s.value})),
                    (None, Some(m), None, None) => Ok(json!({"map":m.value})),
                    (None, None, Some(s), None) => Ok(json!({"store":s.value})),
                    (None, None, None, Some(p)) => Ok(json!({"params":p.value})),
                    _ => bail!("module {} has an input without a single kind", module.name),
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({"name":module.name,"kind":kind,"binary_index":module.binary_index,"inputs":inputs,
                "output":module.kind_map.as_ref().map(|m| m.output_type.clone()).or_else(|| module.output.as_ref().map(|o| o.value.clone())),
                "initial_block":module.initial_block}))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"modules":modules,"binaries":binaries}))
}

/// Imports through which a module could reach chain state outside its block.
pub fn external_state_imports(imports: &[String]) -> Vec<String> {
    imports.iter().filter(|i| i.starts_with("rpc.")).cloned().collect()
}

pub fn inspect(package: &PathBuf, reference: &PathBuf, wasm: Option<&PathBuf>) -> Result<Value> {
    let candidate = decode(package)?;
    let canonical = decode(reference)?;
    let mut report = describe(&candidate)?;
    let modules = report["modules"].as_array().unwrap();
    ensure!(
        modules.len() == 1 && modules[0]["name"] == "map_events",
        "exactly one map_events module required"
    );
    let module = &modules[0];
    ensure!(
        module["kind"] == "map"
            && module["inputs"] == json!([{"params":"[]"},{"source":"sf.ethereum.type.v2.Block"}])
            && module["output"] == "proto:evm.balances.v1.Events",
        "map_events must read the default [] parameter and Extended blocks and emit evm.balances.v1.Events"
    );
    let binary = &report["binaries"][module["binary_index"].as_u64().unwrap() as usize];
    let imports = binary["imports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    ensure!(external_state_imports(&imports).is_empty(), "map_events binary imports an RPC host function");
    if let Some(wasm) = wasm {
        ensure!(binary["sha256"] == sha256(wasm)?, "packaged binary differs from the built WASM");
    }
    let ours = schema(&candidate, "evm.balances.v1")?;
    let theirs = schema(&canonical, "evm.balances.v1")?;
    ensure!(ours == theirs, "evm.balances.v1 descriptor differs from the canonical RPC reference");
    let reference_imports = describe(&canonical)?["binaries"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|b| {
            b["imports"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    report["schema"] = json!({"package":"evm.balances.v1","descriptor_sha256":digest(&ours),"byte_identical_to_reference":true});
    report["reference_rpc_imports"] = json!(external_state_imports(&reference_imports));
    Ok(report)
}

pub fn run(args: InspectPackage) -> Result<bool> {
    record_run(&args.output, json!({"status":"incomplete"}), |report| {
        report["package_sha256"] = json!(sha256(&args.package)?);
        report["reference_sha256"] = json!(sha256(&args.reference)?);
        let inspected = inspect(&args.package, &args.reference, args.wasm.as_ref())?;
        report.as_object_mut().unwrap().extend(inspected.as_object().unwrap().clone());
        report["status"] = json!("inspected");
        Ok(())
    })
}
